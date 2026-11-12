//! Codificação e decodificação do protocolo BLE do QCY HT05.
//!
//! Funções puras, sem I/O. Ver `docs/protocol.md`. Porte 1:1 de `protocol.py`.
//! Este módulo nunca define comandos de OTA nem de restauração de fábrica.

use std::collections::BTreeMap;
use std::fmt;

// --- GATT -------------------------------------------------------------------

/// UUID de 128 bits a partir do UUID curto de 16 bits.
pub fn uuid(short: u16) -> String {
    format!("0000{short:04x}-0000-1000-8000-00805f9b34fb")
}

pub const SERVICE: &str = "0000a001-0000-1000-8000-00805f9b34fb";
/// read: versão L (3 bytes) + R (3 bytes)
pub const CHAR_VERSION: &str = "00000007-0000-1000-8000-00805f9b34fb";
/// read/notify: [L, R, caixa]
pub const CHAR_BATTERY: &str = "00000008-0000-1000-8000-00805f9b34fb";
/// read/write: [tipo, 10 ganhos em dB]
pub const CHAR_EQ: &str = "0000000b-0000-1000-8000-00805f9b34fb";
/// read/write: pares [evento, função]
pub const CHAR_KEYS: &str = "0000000d-0000-1000-8000-00805f9b34fb";
/// write: frames TLV
pub const CHAR_CMD: &str = "00001001-0000-1000-8000-00805f9b34fb";
/// read/notify: frames TLV
pub const CHAR_RESP: &str = "00001002-0000-1000-8000-00805f9b34fb";

/// manufacturer data no anúncio BLE
pub const COMPANY_ID: u16 = 0x521C;
pub const COMPANY_ID_ALT: u16 = 0x522C;
pub const VENDOR_ID_HT05: u16 = 0x4D41;

// --- IDs de comando TLV -----------------------------------------------------

/// IDs de comando TLV.
pub mod cmd {
    pub const RESET_DEFAULT: u8 = 0x01;
    pub const FACTORY_RESET: u8 = 0x03;
    pub const EAR_DETECT: u8 = 0x06;
    pub const VOLUME: u8 = 0x08;
    pub const GAME_MODE: u8 = 0x09;
    pub const NOISE_MODE: u8 = 0x0C;
    pub const SLEEP_MODE: u8 = 0x10;
    pub const BALANCE: u8 = 0x16;
    pub const PAIR_NAME: u8 = 0x18;
    pub const REQUEST: u8 = 0xFE;
}

pub const FRAME_HEADER: u8 = 0xFF;
/// Booleanos do protocolo.
pub const ON: u8 = 0x01;
pub const OFF: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl ProtocolError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtocolError {}

pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Bytes em hexadecimal separados por espaço (como `bytes.hex(' ')`).
fn hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Monta um frame `FF len [id len payload]...`.
pub fn build_frame(tlvs: &[(u8, &[u8])]) -> Result<Vec<u8>> {
    let mut body: Vec<u8> = Vec::new();
    for (cmd_id, payload) in tlvs {
        if payload.len() > 255 {
            return Err(ProtocolError::new("payload grande demais"));
        }
        body.push(*cmd_id);
        body.push(payload.len() as u8);
        body.extend_from_slice(payload);
    }
    if body.len() > 255 {
        return Err(ProtocolError::new("frame grande demais"));
    }
    let mut out = vec![FRAME_HEADER, body.len() as u8];
    out.extend_from_slice(&body);
    Ok(out)
}

/// Separa um frame em uma lista de (id, payload).
pub fn parse_frame(data: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
    if data.len() < 2 || data[0] != FRAME_HEADER || data[1] as usize != data.len() - 2 {
        return Err(ProtocolError::new(format!("frame inválido: {}", hex(data))));
    }
    let mut out = Vec::new();
    let mut i = 2;
    while i + 2 <= data.len() {
        let (cmd_id, length) = (data[i], data[i + 1] as usize);
        let end = (i + 2 + length).min(data.len());
        let payload = &data[i + 2..end];
        if payload.len() != length {
            return Err(ProtocolError::new(format!("TLV truncado: {}", hex(data))));
        }
        out.push((cmd_id, payload.to_vec()));
        i += 2 + length;
    }
    Ok(out)
}

pub fn cmd_set(cmd_id: u8, value: u8) -> Vec<u8> {
    // Um TLV de 1 byte nunca excede os limites do frame.
    vec![FRAME_HEADER, 3, cmd_id, 1, value]
}

/// Pede ao fone o valor atual de um ou mais IDs.
pub fn cmd_request(cmd_ids: &[u8]) -> Result<Vec<u8>> {
    let bufs: Vec<[u8; 1]> = cmd_ids.iter().map(|c| [*c]).collect();
    let tlvs: Vec<(u8, &[u8])> = bufs.iter().map(|b| (cmd::REQUEST, &b[..])).collect();
    build_frame(&tlvs)
}

// --- Modo de ruído ----------------------------------------------------------
// Valores do painel oficial do HT05 (modelId 19777, firmware >= 2.0.9).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NoiseKind {
    Normal = 0,
    Anc = 1,
    Transparency = 2,
}

/// Cenas ANC: (nome interno, rótulo, valor base do nível 1).
pub const ANC_SCENES: [(&str, &str, u8); 4] = [
    ("indoor", "Interno", 0x41),
    ("commute", "Transporte", 0x51),
    ("noisy", "Ruidoso", 0x61),
    ("wind", "Anti-vento", 0x71),
];
/// Níveis 1..3 (exceto "wind", que tem 1).
pub const ANC_LEVELS: u8 = 3;
pub const NOISE_NORMAL: u8 = 0x02;
/// Transparência com realce de voz.
pub const TRANSPARENCY_VOICE: u8 = 0xA0;
pub const TRANSPARENCY_MIN: u8 = 0xA1;
pub const TRANSPARENCY_MAX: u8 = 0xA6;
pub const TRANSPARENCY_DEFAULT: u8 = 0xA4;
pub const ANC_DEFAULT: u8 = 0x63;

fn anc_scene(name: &str) -> Option<&'static (&'static str, &'static str, u8)> {
    ANC_SCENES.iter().find(|s| s.0 == name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseMode {
    pub kind: NoiseKind,
    /// Só para ANC.
    pub scene: Option<&'static str>,
    /// ANC: 1..3; transparência: 1..6, 0 = voz.
    pub level: u8,
}

impl NoiseMode {
    pub const fn new(kind: NoiseKind, scene: Option<&'static str>, level: u8) -> Self {
        Self { kind, scene, level }
    }

    pub const fn normal() -> Self {
        Self::new(NoiseKind::Normal, None, 0)
    }

    pub const fn anc(scene: &'static str, level: u8) -> Self {
        Self::new(NoiseKind::Anc, Some(scene), level)
    }

    pub const fn transparency(level: u8) -> Self {
        Self::new(NoiseKind::Transparency, None, level)
    }

    pub fn encode(&self) -> Result<u8> {
        match self.kind {
            NoiseKind::Normal => Ok(NOISE_NORMAL),
            NoiseKind::Transparency => {
                if self.level == 0 {
                    return Ok(TRANSPARENCY_VOICE);
                }
                if self.level > 6 {
                    return Err(ProtocolError::new("nível de transparência deve ser 0..6"));
                }
                Ok(TRANSPARENCY_MIN + self.level - 1)
            }
            NoiseKind::Anc => {
                let scene = self.scene.and_then(anc_scene).ok_or_else(|| {
                    ProtocolError::new(format!(
                        "cena ANC desconhecida: {}",
                        self.scene.unwrap_or("None")
                    ))
                })?;
                let max_level = if scene.0 == "wind" { 1 } else { ANC_LEVELS };
                if !(1..=max_level).contains(&self.level) {
                    return Err(ProtocolError::new(format!(
                        "nível ANC deve ser 1..{max_level}"
                    )));
                }
                Ok(scene.2 + self.level - 1)
            }
        }
    }

    pub fn decode(value: u8) -> Result<Self> {
        if value == NOISE_NORMAL {
            return Ok(Self::normal());
        }
        if value == TRANSPARENCY_VOICE {
            return Ok(Self::transparency(0));
        }
        if (TRANSPARENCY_MIN..=TRANSPARENCY_MAX).contains(&value) {
            return Ok(Self::transparency(value - TRANSPARENCY_MIN + 1));
        }
        for (name, _, base) in ANC_SCENES.iter() {
            if *base <= value && (value as u16) < *base as u16 + ANC_LEVELS as u16 {
                return Ok(Self::anc(name, value - base + 1));
            }
        }
        Err(ProtocolError::new(format!(
            "modo de ruído desconhecido: 0x{value:02x}"
        )))
    }

    pub fn label(&self) -> String {
        match self.kind {
            NoiseKind::Normal => "Normal".to_string(),
            NoiseKind::Transparency => {
                if self.level == 0 {
                    "Transparência (voz)".to_string()
                } else {
                    format!("Transparência {}", self.level)
                }
            }
            NoiseKind::Anc => {
                let nome = self
                    .scene
                    .and_then(anc_scene)
                    .map(|s| s.1)
                    .unwrap_or("?");
                format!("ANC {nome} {}", self.level)
            }
        }
    }
}

pub fn cmd_noise(mode: &NoiseMode) -> Result<Vec<u8>> {
    Ok(cmd_set(cmd::NOISE_MODE, mode.encode()?))
}

// --- Nome de pareamento (TLV 0x18) -----------------------------------------

pub const PAIR_NAME_MAX_BYTES: usize = 32;

/// Novo nome Bluetooth. O fone aplica o nome na próxima conexão clássica.
pub fn cmd_rename(name: &str) -> Result<Vec<u8>> {
    let raw = name.as_bytes();
    if name.trim().is_empty() {
        return Err(ProtocolError::new("o nome não pode ficar vazio"));
    }
    if raw.len() > PAIR_NAME_MAX_BYTES {
        return Err(ProtocolError::new(format!(
            "nome com mais de {PAIR_NAME_MAX_BYTES} bytes UTF-8"
        )));
    }
    build_frame(&[(cmd::PAIR_NAME, raw)])
}

/// O fone devolve o nome em 32 bytes completados com zeros.
pub fn decode_name(payload: &[u8]) -> String {
    let end = payload.iter().position(|b| *b == 0).unwrap_or(payload.len());
    String::from_utf8_lossy(&payload[..end]).into_owned()
}

// --- Bateria e versão -------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryLevel {
    /// None = desconhecido (fora da caixa, desligado).
    pub percent: Option<u8>,
    pub charging: bool,
}

impl BatteryLevel {
    pub fn decode(byte: u8) -> Self {
        let pct = byte & 0x7F;
        Self {
            percent: if (1..=100).contains(&pct) { Some(pct) } else { None },
            charging: byte & 0x80 != 0,
        }
    }

    pub fn label(&self) -> String {
        match self.percent {
            None => "—".to_string(),
            Some(p) => format!("{p}%{}", if self.charging { " ⚡" } else { "" }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub left: BatteryLevel,
    pub right: BatteryLevel,
    pub case: BatteryLevel,
}

impl Battery {
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 3 {
            return Err(ProtocolError::new(format!(
                "bateria inválida: {}",
                hex(data)
            )));
        }
        Ok(Self {
            left: BatteryLevel::decode(data[0]),
            right: BatteryLevel::decode(data[1]),
            case: BatteryLevel::decode(data[2]),
        })
    }
}

/// Retorna (versão esquerda, versão direita).
pub fn decode_version(data: &[u8]) -> (String, String) {
    let join = |s: &[u8]| {
        s.iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(".")
    };
    let left = join(&data[..data.len().min(3)]);
    let right = if data.len() >= 6 {
        join(&data[3..6])
    } else {
        left.clone()
    };
    (left, right)
}

// --- Equalizador (characteristic 000B) -------------------------------------

pub const EQ_BANDS: [&str; 10] = [
    "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
];
pub const EQ_MIN_DB: i32 = -8;
pub const EQ_MAX_DB: i32 = 8;
pub const EQ_CUSTOM: u8 = 0xFF;

/// Presets internos do firmware: o índice é o tipo; os ganhos são só para exibição.
pub const EQ_PRESETS: [(&str, [i32; 10]); 6] = [
    ("Padrão", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("Pop", [-6, 5, -3, -2, 5, 4, -4, -3, 6, 4]),
    ("Graves", [6, 5, 4, 3, 0, 0, 0, 0, 0, 0]),
    ("Rock", [5, 4, 2, 1, -3, -4, 2, 1, 4, 5]),
    ("Suave", [5, -5, -4, -4, 3, 2, 4, 4, 0, 0]),
    ("Clássico", [5, 5, 1, 2, -1, 1, -4, -6, -7, -8]),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equalizer {
    /// 0..5 ou `EQ_CUSTOM`.
    pub preset: u8,
    /// 10 valores em dB.
    pub gains: Vec<i32>,
}

impl Equalizer {
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 1 + EQ_BANDS.len() {
            return Err(ProtocolError::new(format!("EQ inválido: {}", hex(data))));
        }
        let preset = data[0];
        let mut gains: Vec<i32> = data[1..11].iter().map(|b| *b as i8 as i32).collect();
        if (preset as usize) < EQ_PRESETS.len() {
            // O firmware devolve ganhos zerados para presets internos.
            gains = EQ_PRESETS[preset as usize].1.to_vec();
        }
        Ok(Self { preset, gains })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.preset != EQ_CUSTOM {
            if self.preset as usize >= EQ_PRESETS.len() {
                return Err(ProtocolError::new("preset de EQ inválido"));
            }
            let mut out = vec![self.preset];
            out.extend_from_slice(&[0u8; 10]);
            return Ok(out);
        }
        if self.gains.len() != EQ_BANDS.len() {
            return Err(ProtocolError::new("EQ precisa de 10 ganhos"));
        }
        if self.gains.iter().any(|g| !(EQ_MIN_DB..=EQ_MAX_DB).contains(g)) {
            return Err(ProtocolError::new(format!(
                "ganho fora de {EQ_MIN_DB}..{EQ_MAX_DB} dB"
            )));
        }
        let mut out = vec![EQ_CUSTOM];
        out.extend(self.gains.iter().map(|g| (*g & 0xFF) as u8));
        Ok(out)
    }

    pub fn custom(gains: &[i32]) -> Self {
        Self {
            preset: EQ_CUSTOM,
            gains: gains.to_vec(),
        }
    }

    pub fn label(&self) -> &'static str {
        if self.preset == EQ_CUSTOM {
            "Personalizado"
        } else {
            // O Python lança IndexError aqui; em Rust devolve "?" em vez de entrar em pânico.
            EQ_PRESETS.get(self.preset as usize).map_or("?", |p| p.0)
        }
    }
}

// --- Controles de toque (characteristic 000D) -------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Gesture {
    TapLeft = 1,
    TapRight = 2,
    DoubleLeft = 3,
    DoubleRight = 4,
    TripleLeft = 5,
    TripleRight = 6,
}

impl Gesture {
    pub const ALL: [Gesture; 6] = [
        Gesture::TapLeft,
        Gesture::TapRight,
        Gesture::DoubleLeft,
        Gesture::DoubleRight,
        Gesture::TripleLeft,
        Gesture::TripleRight,
    ];

    /// (tipo de toque, lado)
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            Gesture::TapLeft => ("Toque", "Esquerdo"),
            Gesture::TapRight => ("Toque", "Direito"),
            Gesture::DoubleLeft => ("Toque duplo", "Esquerdo"),
            Gesture::DoubleRight => ("Toque duplo", "Direito"),
            Gesture::TripleLeft => ("Toque triplo", "Esquerdo"),
            Gesture::TripleRight => ("Toque triplo", "Direito"),
        }
    }
}

/// Funções de toque: (código, rótulo).
pub const KEY_ACTIONS: [(u8, &str); 9] = [
    (0, "Nenhuma"),
    (1, "Play/Pausa"),
    (2, "Faixa anterior"),
    (3, "Próxima faixa"),
    (4, "Assistente de voz"),
    (5, "Volume +"),
    (6, "Volume −"),
    (7, "Modo jogo"),
    (11, "Alternar ANC"),
];

/// Rótulo de uma função de toque, se conhecida.
pub fn key_action_label(action: u8) -> Option<&'static str> {
    KEY_ACTIONS.iter().find(|a| a.0 == action).map(|a| a.1)
}

/// Retorna {evento: função}. Ignora pares vazios (evento 0).
pub fn decode_keys(data: &[u8]) -> BTreeMap<u8, u8> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let (event, func) = (data[i], data[i + 1]);
        if event != 0 {
            out.insert(event, func);
        }
        i += 2;
    }
    out
}

pub fn encode_key(gesture: u8, action: u8) -> Result<Vec<u8>> {
    if key_action_label(action).is_none() {
        return Err(ProtocolError::new(format!(
            "ação de toque desconhecida: {action}"
        )));
    }
    Ok(vec![gesture, action])
}

// --- Anúncio BLE ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advertisement {
    pub vendor_id: u16,
    pub battery: Battery,
    pub classic_mac: String,
}

/// Lê o manufacturer data (sem o company ID) do anúncio do fone.
pub fn parse_advertisement(payload: &[u8]) -> Option<Advertisement> {
    if payload.len() < 17 {
        return None;
    }
    let vendor = ((payload[0] as u16) << 8) | payload[1] as u16;
    let battery = Battery::decode(&payload[5..8]).ok()?;
    let mac = [
        payload[12],
        payload[11],
        payload[13],
        payload[16],
        payload[15],
        payload[14],
    ];
    let classic_mac = mac
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    Some(Advertisement {
        vendor_id: vendor,
        battery,
        classic_mac,
    })
}

#[cfg(test)]
mod tests {
    //! Testes com pacotes reais capturados do HT05 (firmware 4.0.0).
    use super::*;

    // Pacotes reais lidos do fone em 2026-11-12.
    const STATE_FRAME: &str = "ff1406010108037f7f0f0901020c0163100100160132";
    const ADV_PAYLOAD: &str = "4d41ae806a646400000000ac846033221100000000000000";

    fn unhex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn name_frame() -> Vec<u8> {
        // "Pedrot " + zeros
        let mut v = unhex("ff221820506564726f7420");
        v.extend(vec![0u8; 25]);
        v
    }

    #[test]
    fn parse_state_frame() {
        let tlvs: BTreeMap<u8, Vec<u8>> = parse_frame(&unhex(STATE_FRAME)).unwrap().into_iter().collect();
        assert_eq!(tlvs[&cmd::NOISE_MODE], vec![0x63]);
        assert_eq!(tlvs[&cmd::GAME_MODE], vec![0x02]);
        assert_eq!(tlvs[&cmd::BALANCE], vec![0x32]);
        assert_eq!(tlvs[&cmd::VOLUME], vec![0x7F, 0x7F, 0x0F]);
    }

    #[test]
    fn parse_frame_rejects_invalid() {
        for bad in [
            &b""[..],
            &b"\xff"[..],
            &b"\xfe\x01\x00"[..],
            &b"\xff\x05\x0c\x01\x02"[..],
        ] {
            assert!(parse_frame(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parse_frame_rejects_truncated_tlv() {
        // O tamanho do frame confere, mas o TLV declara mais bytes do que existem.
        assert!(parse_frame(&[0xFF, 3, 0x0C, 5, 0x01]).is_err());
    }

    #[test]
    fn build_frame_roundtrip() {
        let frame = build_frame(&[(0x0C, &[0x02]), (0x09, &[0x01])]).unwrap();
        assert_eq!(frame, unhex("ff060c0102090101"));
        assert_eq!(
            parse_frame(&frame).unwrap(),
            vec![(0x0C, vec![0x02]), (0x09, vec![0x01])]
        );
    }

    #[test]
    fn commands_match_official_app() {
        // Valores do painel oficial e do teste no fone.
        assert_eq!(cmd_set(cmd::GAME_MODE, ON), unhex("ff03090101"));
        assert_eq!(cmd_set(cmd::GAME_MODE, OFF), unhex("ff03090102"));
        assert_eq!(cmd_set(cmd::SLEEP_MODE, ON), unhex("ff03100101"));
        assert_eq!(cmd_request(&[cmd::NOISE_MODE]).unwrap(), unhex("ff03fe010c"));
    }

    #[test]
    fn noise_mode_roundtrip() {
        let casos = [
            (0x02, NoiseMode::normal()),
            (0x41, NoiseMode::anc("indoor", 1)),
            (0x53, NoiseMode::anc("commute", 3)),
            (0x63, NoiseMode::anc("noisy", 3)),
            (0x71, NoiseMode::anc("wind", 1)),
            (0xA0, NoiseMode::transparency(0)),
            (0xA4, NoiseMode::transparency(4)),
            (0xA6, NoiseMode::transparency(6)),
        ];
        for (value, mode) in casos {
            assert_eq!(NoiseMode::decode(value).unwrap(), mode);
            assert_eq!(mode.encode().unwrap(), value);
        }
    }

    #[test]
    fn noise_mode_rejects_invalid() {
        for mode in [
            NoiseMode::anc("noisy", 4),
            NoiseMode::anc("wind", 2),
            NoiseMode::anc("space", 1),
            NoiseMode::transparency(7),
        ] {
            assert!(mode.encode().is_err(), "{mode:?}");
        }
    }

    #[test]
    fn noise_decode_unknown() {
        assert!(NoiseMode::decode(0x30).is_err());
    }

    #[test]
    fn noise_labels() {
        assert_eq!(NoiseMode::normal().label(), "Normal");
        assert_eq!(NoiseMode::anc("noisy", 3).label(), "ANC Ruidoso 3");
        assert_eq!(NoiseMode::transparency(0).label(), "Transparência (voz)");
        assert_eq!(NoiseMode::transparency(2).label(), "Transparência 2");
        assert_eq!(
            cmd_noise(&NoiseMode::anc("noisy", 3)).unwrap(),
            unhex("ff030c0163")
        );
    }

    #[test]
    fn battery_decode() {
        let b = Battery::decode(&[0x63, 0x64 | 0x80, 0x00]).unwrap();
        assert_eq!(b.left.percent, Some(99));
        assert!(!b.left.charging);
        assert_eq!(b.right.percent, Some(100));
        assert!(b.right.charging);
        assert_eq!(b.case.percent, None);
        assert_eq!(b.right.label(), "100% ⚡");
        assert_eq!(b.case.label(), "—");
        assert!(Battery::decode(&[1, 2]).is_err());
    }

    #[test]
    fn version_decode() {
        assert_eq!(
            decode_version(&[4, 0, 0, 4, 0, 1]),
            ("4.0.0".to_string(), "4.0.1".to_string())
        );
        assert_eq!(
            decode_version(&[4, 0, 2]),
            ("4.0.2".to_string(), "4.0.2".to_string())
        );
    }

    #[test]
    fn eq_custom_roundtrip() {
        let raw = unhex("ff0302010000000000 0102");
        let eq = Equalizer::decode(&raw).unwrap();
        assert_eq!(eq.preset, EQ_CUSTOM);
        assert_eq!(eq.gains, vec![3, 2, 1, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(eq.encode().unwrap(), raw);
    }

    #[test]
    fn eq_negative_gains() {
        let eq = Equalizer::custom(&[-8, -1, 0, 0, 0, 0, 0, 0, 1, 8]);
        assert_eq!(
            Equalizer::decode(&eq.encode().unwrap()).unwrap().gains,
            vec![-8, -1, 0, 0, 0, 0, 0, 0, 1, 8]
        );
    }

    #[test]
    fn eq_preset_uses_table_gains() {
        // O firmware devolve ganhos zerados para presets internos.
        let mut raw = vec![1u8];
        raw.extend([0u8; 10]);
        let eq = Equalizer::decode(&raw).unwrap();
        assert_eq!(eq.label(), "Pop");
        assert_eq!(eq.gains, EQ_PRESETS[1].1.to_vec());
        let mut esperado = vec![3u8];
        esperado.extend([0u8; 10]);
        assert_eq!(
            Equalizer { preset: 3, gains: vec![] }.encode().unwrap(),
            esperado
        );
    }

    #[test]
    fn eq_rejects_out_of_range() {
        let mut g = vec![9];
        g.extend([0; 9]);
        assert!(Equalizer::custom(&g).encode().is_err());
        assert!(Equalizer::custom(&[0; 9]).encode().is_err());
        assert!(Equalizer { preset: 6, gains: vec![] }.encode().is_err());
    }

    #[test]
    fn keys_decode_real_value() {
        let raw = unhex("0101020103020403050706 0b07000800000000 00");
        let keys = decode_keys(&raw);
        assert_eq!(keys[&(Gesture::TapLeft as u8)], 1);
        assert_eq!(keys[&(Gesture::DoubleRight as u8)], 3);
        assert_eq!(keys[&(Gesture::TripleLeft as u8)], 7);
        assert_eq!(keys[&(Gesture::TripleRight as u8)], 11);
        assert_eq!(encode_key(Gesture::TripleRight as u8, 11).unwrap(), vec![6, 0x0b]);
        assert!(encode_key(1, 9).is_err());
    }

    #[test]
    fn gesture_labels() {
        assert_eq!(Gesture::DoubleLeft.label(), ("Toque duplo", "Esquerdo"));
        assert_eq!(key_action_label(11), Some("Alternar ANC"));
    }

    #[test]
    fn rename() {
        let frame = cmd_rename("Meu HT05").unwrap();
        let mut esperado = vec![0xFF, 10, 0x18, 8];
        esperado.extend(b"Meu HT05");
        assert_eq!(frame, esperado);
        assert_eq!(cmd_rename("Fone ção").unwrap()[3] as usize, "Fone ção".len());
        assert!(cmd_rename("   ").is_err());
        assert!(cmd_rename(&"x".repeat(33)).is_err());
    }

    #[test]
    fn decode_name_padded() {
        let tlvs: BTreeMap<u8, Vec<u8>> = parse_frame(&name_frame()).unwrap().into_iter().collect();
        assert_eq!(decode_name(&tlvs[&cmd::PAIR_NAME]), "Pedrot ");
    }

    #[test]
    fn advertisement() {
        let ad = parse_advertisement(&unhex(ADV_PAYLOAD)).unwrap();
        assert_eq!(ad.vendor_id, VENDOR_ID_HT05);
        assert_eq!(ad.classic_mac, "84:AC:60:11:22:33");
        assert_eq!(ad.battery.left.percent, Some(100));
        assert!(parse_advertisement(&[0u8; 5]).is_none());
    }

    #[test]
    fn uuids() {
        assert_eq!(uuid(0xA001), SERVICE);
        assert_eq!(uuid(0x0007), CHAR_VERSION);
        assert_eq!(uuid(0x0008), CHAR_BATTERY);
        assert_eq!(uuid(0x000B), CHAR_EQ);
        assert_eq!(uuid(0x000D), CHAR_KEYS);
        assert_eq!(uuid(0x1001), CHAR_CMD);
        assert_eq!(uuid(0x1002), CHAR_RESP);
    }
}
