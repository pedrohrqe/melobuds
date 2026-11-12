//! Leitor mínimo, somente leitura, do formato de hive do registro do Windows ("regf").
//!
//! Implementado a partir da documentação pública do formato. Usa só a biblioteca padrão.
//! Todo deslocamento é validado: entrada corrompida ou truncada devolve `Err`, nunca causa pânico.

use std::fmt;

/// Tipo de valor REG_BINARY.
pub const REG_BINARY: u32 = 3;
/// Tipo de valor REG_DWORD.
pub const REG_DWORD: u32 = 4;

/// Tamanho do bloco base. Os deslocamentos das células contam a partir do fim dele.
const BASE_BLOCK: usize = 0x1000;
/// Maior tamanho de dados de um segmento de célula `db`.
const MAX_SEGMENT: usize = 16344;
/// Limite de segurança para o tamanho de um valor (evita alocação absurda em hive corrompido).
const MAX_VALUE_LEN: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegfError(pub String);

impl fmt::Display for RegfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RegfError {}

pub type Result<T> = std::result::Result<T, RegfError>;

fn bad<T>(msg: &str) -> Result<T> {
    Err(RegfError(msg.to_string()))
}

fn u16_at(b: &[u8], pos: usize) -> Result<u16> {
    let s = b.get(pos..pos.checked_add(2).ok_or(RegfError("deslocamento inválido".into()))?);
    s.map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| RegfError("leitura fora dos limites".into()))
}

fn u32_at(b: &[u8], pos: usize) -> Result<u32> {
    let s = b.get(pos..pos.checked_add(4).ok_or(RegfError("deslocamento inválido".into()))?);
    s.map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| RegfError("leitura fora dos limites".into()))
}

/// Hive carregado em memória.
#[derive(Debug, Clone, Copy)]
pub struct Hive<'a> {
    data: &'a [u8],
    root: u32,
}

/// Valor de registro: nome, tipo e bytes brutos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    pub name: String,
    pub data_type: u32,
    pub data: Vec<u8>,
}

impl Value {
    /// Valor REG_DWORD como número, se o tipo e o tamanho estiverem corretos.
    pub fn dword(&self) -> Option<u32> {
        if self.data_type != REG_DWORD {
            return None;
        }
        <[u8; 4]>::try_from(self.data.as_slice()).ok().map(u32::from_le_bytes)
    }
}

/// Chave de registro (célula `nk`).
#[derive(Debug, Clone)]
pub struct Key<'a> {
    hive: Hive<'a>,
    name: String,
    subkey_count: u32,
    subkey_list: u32,
    value_count: u32,
    value_list: u32,
}

impl<'a> Hive<'a> {
    /// Abre um hive a partir dos bytes do arquivo. Valida só a assinatura "regf".
    pub fn new(data: &'a [u8]) -> Result<Self> {
        if data.len() < BASE_BLOCK {
            return bad("arquivo menor que o bloco base do hive");
        }
        if &data[0..4] != b"regf" {
            return bad("assinatura regf ausente");
        }
        let root = u32_at(data, 0x24)?;
        let hive = Hive { data, root };
        hive.cell(root)?; // garante que a célula raiz existe
        Ok(hive)
    }

    /// Conteúdo (sem o campo de tamanho) da célula no deslocamento relativo `off`.
    fn cell(&self, off: u32) -> Result<&'a [u8]> {
        let start = BASE_BLOCK.checked_add(off as usize).ok_or(RegfError("deslocamento inválido".into()))?;
        let size = i32::from_le_bytes(
            self.data
                .get(start..start.saturating_add(4))
                .and_then(|s| <[u8; 4]>::try_from(s).ok())
                .ok_or_else(|| RegfError("célula fora dos limites".into()))?,
        )
        .unsigned_abs() as usize;
        if size < 4 {
            return bad("tamanho de célula inválido");
        }
        let end = start.checked_add(size).ok_or(RegfError("tamanho de célula inválido".into()))?;
        self.data.get(start + 4..end).ok_or_else(|| RegfError("célula ultrapassa o arquivo".into()))
    }

    /// Chave raiz do hive.
    pub fn root(&self) -> Result<Key<'a>> {
        self.key_at(self.root)
    }

    fn key_at(&self, off: u32) -> Result<Key<'a>> {
        let c = self.cell(off)?;
        if c.get(0..2) != Some(b"nk") {
            return bad("célula nk esperada");
        }
        let flags = u16_at(c, 2)?;
        let name_len = u16_at(c, 72)? as usize;
        let raw = c.get(76..76 + name_len).ok_or_else(|| RegfError("nome da chave fora dos limites".into()))?;
        Ok(Key {
            hive: *self,
            name: decode_name(raw, flags & 0x0020 != 0)?,
            subkey_count: u32_at(c, 20)?,
            subkey_list: u32_at(c, 28)?,
            value_count: u32_at(c, 36)?,
            value_list: u32_at(c, 40)?,
        })
    }

    /// Deslocamentos das chaves filhas, expandindo listas lf/lh/li/ri.
    fn subkey_offsets(&self, list: u32, allow_ri: bool, out: &mut Vec<u32>) -> Result<()> {
        let c = self.cell(list)?;
        let count = u16_at(c, 2)? as usize;
        let sig = c.get(0..2).unwrap_or(&[]);
        let (stride, is_ri) = match sig {
            b"lf" | b"lh" => (8, false),
            b"li" => (4, false),
            b"ri" => (4, true),
            _ => return bad("lista de subchaves inválida"),
        };
        if is_ri && !allow_ri {
            return bad("lista ri aninhada");
        }
        for i in 0..count {
            let off = u32_at(c, 4 + i * stride)?;
            if is_ri {
                self.subkey_offsets(off, false, out)?;
            } else {
                out.push(off);
            }
        }
        Ok(())
    }

    fn value_at(&self, off: u32) -> Result<Value> {
        let c = self.cell(off)?;
        if c.get(0..2) != Some(b"vk") {
            return bad("célula vk esperada");
        }
        let name_len = u16_at(c, 2)? as usize;
        let raw_len = u32_at(c, 4)?;
        let data_off = u32_at(c, 8)?;
        let data_type = u32_at(c, 12)?;
        let flags = u16_at(c, 16)?;
        let raw = c.get(20..20 + name_len).ok_or_else(|| RegfError("nome do valor fora dos limites".into()))?;
        let name = decode_name(raw, flags & 0x0001 != 0)?;
        let data = if raw_len & 0x8000_0000 != 0 {
            // Dados embutidos no próprio campo de deslocamento (até 4 bytes).
            let len = (raw_len & 0x7FFF_FFFF) as usize;
            if len > 4 {
                return bad("dados embutidos maiores que 4 bytes");
            }
            data_off.to_le_bytes()[..len].to_vec()
        } else {
            self.read_data(data_off, raw_len as usize)?
        };
        Ok(Value { name, data_type, data })
    }

    fn read_data(&self, off: u32, len: usize) -> Result<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }
        if len > MAX_VALUE_LEN {
            return bad("valor grande demais");
        }
        let c = self.cell(off)?;
        if len > MAX_SEGMENT && c.get(0..2) == Some(b"db") {
            let segments = u16_at(c, 2)? as usize;
            let list = self.cell(u32_at(c, 4)?)?;
            let mut out = Vec::with_capacity(len);
            for i in 0..segments {
                let seg = self.cell(u32_at(list, i * 4)?)?;
                let take = (len - out.len()).min(MAX_SEGMENT).min(seg.len());
                out.extend_from_slice(&seg[..take]);
                if out.len() == len {
                    break;
                }
            }
            if out.len() != len {
                return bad("dados da célula db incompletos");
            }
            return Ok(out);
        }
        c.get(..len).map(<[u8]>::to_vec).ok_or_else(|| RegfError("dados do valor fora dos limites".into()))
    }
}

/// Decodifica nome ASCII (comprimido, um byte por caractere) ou UTF-16LE.
fn decode_name(raw: &[u8], compressed: bool) -> Result<String> {
    if compressed {
        return Ok(raw.iter().map(|&b| b as char).collect());
    }
    if !raw.len().is_multiple_of(2) {
        return bad("nome UTF-16 com tamanho ímpar");
    }
    let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|p| u16::from_le_bytes(*p)).collect();
    Ok(String::from_utf16_lossy(&units))
}

impl<'a> Key<'a> {
    /// Nome da chave.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Chaves filhas, na ordem da lista.
    pub fn subkeys(&self) -> Result<Vec<Key<'a>>> {
        if self.subkey_count == 0 {
            return Ok(Vec::new());
        }
        let mut offsets = Vec::new();
        self.hive.subkey_offsets(self.subkey_list, true, &mut offsets)?;
        offsets.into_iter().map(|o| self.hive.key_at(o)).collect()
    }

    /// Chave filha pelo nome, sem diferenciar maiúsculas de minúsculas.
    pub fn subkey(&self, name: &str) -> Result<Option<Key<'a>>> {
        let want = name.to_lowercase();
        Ok(self.subkeys()?.into_iter().find(|k| k.name.to_lowercase() == want))
    }

    /// Segue um caminho separado por `\` a partir desta chave.
    pub fn subpath(&self, path: &str) -> Result<Option<Key<'a>>> {
        let mut cur = self.clone();
        for part in path.split('\\').filter(|p| !p.is_empty()) {
            match cur.subkey(part)? {
                Some(k) => cur = k,
                None => return Ok(None),
            }
        }
        Ok(Some(cur))
    }

    /// Valores da chave.
    pub fn values(&self) -> Result<Vec<Value>> {
        if self.value_count == 0 {
            return Ok(Vec::new());
        }
        let list = self.hive.cell(self.value_list)?;
        (0..self.value_count as usize).map(|i| self.hive.value_at(u32_at(list, i * 4)?)).collect()
    }

    /// Valor pelo nome, sem diferenciar maiúsculas de minúsculas.
    pub fn value(&self, name: &str) -> Result<Option<Value>> {
        let want = name.to_lowercase();
        Ok(self.values()?.into_iter().find(|v| v.name.to_lowercase() == want))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta um hive sintético: bloco base + um hbin com células.
    struct Builder {
        buf: Vec<u8>,
    }

    impl Builder {
        fn new() -> Self {
            let mut buf = vec![0u8; 0x20];
            buf[0..4].copy_from_slice(b"hbin");
            Builder { buf }
        }

        /// Adiciona uma célula alocada e devolve o deslocamento relativo.
        fn cell(&mut self, payload: &[u8]) -> u32 {
            let off = self.buf.len() as u32;
            let size = (4 + payload.len()).div_ceil(8) * 8;
            self.buf.extend_from_slice(&(-(size as i32)).to_le_bytes());
            self.buf.extend_from_slice(payload);
            self.buf.resize(off as usize + size, 0);
            off
        }

        fn nk(&mut self, name: &str, utf16: bool, sub: (u32, u32), vals: (u32, u32)) -> u32 {
            let mut c = vec![0u8; 76];
            c[0..2].copy_from_slice(b"nk");
            let raw: Vec<u8> = if utf16 { name.encode_utf16().flat_map(u16::to_le_bytes).collect() } else { name.bytes().collect() };
            let flags: u16 = if utf16 { 0 } else { 0x20 };
            c[2..4].copy_from_slice(&flags.to_le_bytes());
            c[20..24].copy_from_slice(&sub.0.to_le_bytes());
            c[28..32].copy_from_slice(&sub.1.to_le_bytes());
            c[36..40].copy_from_slice(&vals.0.to_le_bytes());
            c[40..44].copy_from_slice(&vals.1.to_le_bytes());
            c[72..74].copy_from_slice(&(raw.len() as u16).to_le_bytes());
            c.extend_from_slice(&raw);
            self.cell(&c)
        }

        fn list(&mut self, sig: &[u8; 2], offs: &[u32], stride: usize) -> u32 {
            let mut c = sig.to_vec();
            c.extend_from_slice(&(offs.len() as u16).to_le_bytes());
            for o in offs {
                c.extend_from_slice(&o.to_le_bytes());
                if stride == 8 {
                    c.extend_from_slice(&0u32.to_le_bytes());
                }
            }
            self.cell(&c)
        }

        fn offsets(&mut self, offs: &[u32]) -> u32 {
            let c: Vec<u8> = offs.iter().flat_map(|o| o.to_le_bytes()).collect();
            self.cell(&c)
        }

        /// `raw_len` e `data_off` são gravados como estão, para permitir dados embutidos.
        fn vk(&mut self, name: &str, ty: u32, raw_len: u32, data_off: u32) -> u32 {
            let mut c = vec![0u8; 20];
            c[0..2].copy_from_slice(b"vk");
            c[2..4].copy_from_slice(&(name.len() as u16).to_le_bytes());
            c[4..8].copy_from_slice(&raw_len.to_le_bytes());
            c[8..12].copy_from_slice(&data_off.to_le_bytes());
            c[12..16].copy_from_slice(&ty.to_le_bytes());
            c[16..18].copy_from_slice(&1u16.to_le_bytes());
            c.extend_from_slice(name.as_bytes());
            self.cell(&c)
        }

        fn finish(self, root: u32) -> Vec<u8> {
            let mut out = vec![0u8; BASE_BLOCK];
            out[0..4].copy_from_slice(b"regf");
            out[0x24..0x28].copy_from_slice(&root.to_le_bytes());
            out[0x28..0x2C].copy_from_slice(&(self.buf.len() as u32).to_le_bytes());
            out.extend_from_slice(&self.buf);
            out
        }
    }

    /// Hive de exemplo:
    /// ROOT { Select{Current=2 (inline)}, ControlSet002 (UTF-16){ Dados{Bin (fora da célula), Grande (db)} } }
    fn sample() -> Vec<u8> {
        let mut b = Builder::new();
        let cur = b.vk("Current", REG_DWORD, 0x8000_0004, 2);
        let cur_list = b.offsets(&[cur]);
        let select = b.nk("Select", false, (0, 0), (1, cur_list));

        let bin_data = b.cell(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let bin = b.vk("Bin", REG_BINARY, 9, bin_data);
        // Valor grande: 20000 bytes em 2 segmentos.
        let big: Vec<u8> = (0..20000u32).map(|i| (i % 251) as u8).collect();
        let s1 = b.cell(&big[..MAX_SEGMENT]);
        let s2 = b.cell(&big[MAX_SEGMENT..]);
        let seg_list = b.offsets(&[s1, s2]);
        let mut db = b"db".to_vec();
        db.extend_from_slice(&2u16.to_le_bytes());
        db.extend_from_slice(&seg_list.to_le_bytes());
        let db_off = b.cell(&db);
        let grande = b.vk("Grande", REG_BINARY, 20000, db_off);
        let vlist = b.offsets(&[bin, grande]);
        let dados = b.nk("Dados", false, (0, 0), (2, vlist));

        let dados_list = b.list(b"lh", &[dados], 8);
        let cs = b.nk("ControlSet002", true, (1, dados_list), (0, 0));

        let li = b.list(b"li", &[select], 4);
        let lf = b.list(b"lf", &[cs], 8);
        let ri = b.list(b"ri", &[li, lf], 4);
        let root = b.nk("ROOT", false, (2, ri), (0, 0));
        b.finish(root)
    }

    #[test]
    fn caminho_ignora_maiusculas() {
        let data = sample();
        let hive = Hive::new(&data).unwrap();
        let root = hive.root().unwrap();
        assert_eq!(root.name(), "ROOT");
        let k = root.subpath("controlset002\\DADOS").unwrap().unwrap();
        assert_eq!(k.name(), "Dados");
        assert!(root.subpath("ControlSet002\\Nada").unwrap().is_none());
        assert!(root.subkey("nada").unwrap().is_none());
    }

    #[test]
    fn lista_subchaves_com_ri_li_lf() {
        let data = sample();
        let root = Hive::new(&data).unwrap().root().unwrap();
        let names: Vec<String> = root.subkeys().unwrap().iter().map(|k| k.name().to_string()).collect();
        assert_eq!(names, ["Select", "ControlSet002"]);
    }

    #[test]
    fn le_valor_embutido_dword() {
        let data = sample();
        let root = Hive::new(&data).unwrap().root().unwrap();
        let sel = root.subkey("Select").unwrap().unwrap();
        let v = sel.value("current").unwrap().unwrap();
        assert_eq!(v.data_type, REG_DWORD);
        assert_eq!(v.dword(), Some(2));
    }

    #[test]
    fn le_valor_fora_da_celula_e_db() {
        let data = sample();
        let root = Hive::new(&data).unwrap().root().unwrap();
        let k = root.subpath("ControlSet002\\Dados").unwrap().unwrap();
        let vals = k.values().unwrap();
        assert_eq!(vals.len(), 2);
        assert_eq!(vals[0].name, "Bin");
        assert_eq!(vals[0].data, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let expected: Vec<u8> = (0..20000u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(vals[1].data, expected);
        assert_eq!(vals[1].dword(), None);
    }

    #[test]
    fn nome_utf16() {
        let mut b = Builder::new();
        let root = b.nk("Raiz ção", true, (0, 0), (0, 0));
        let data = b.finish(root);
        assert_eq!(Hive::new(&data).unwrap().root().unwrap().name(), "Raiz ção");
    }

    #[test]
    fn entrada_invalida_retorna_erro() {
        assert!(Hive::new(&[]).is_err());
        assert!(Hive::new(&vec![0u8; BASE_BLOCK]).is_err()); // sem assinatura
        let data = sample();
        // Truncado antes da célula raiz.
        assert!(Hive::new(&data[..BASE_BLOCK + 8]).is_err());
        // Raiz apontando para fora do arquivo.
        let mut c = data.clone();
        c[0x24..0x28].copy_from_slice(&0x7FFF_FFF0u32.to_le_bytes());
        assert!(Hive::new(&c).is_err());
    }

    /// Percorre tudo o que o leitor consegue alcançar; o resultado não importa, só não pode dar pânico.
    fn walk(data: &[u8]) {
        fn rec(k: &Key<'_>, depth: usize) {
            let _ = k.values();
            if depth > 8 {
                return;
            }
            if let Ok(subs) = k.subkeys() {
                for s in &subs {
                    rec(s, depth + 1);
                }
            }
        }
        if let Ok(h) = Hive::new(data) {
            if let Ok(r) = h.root() {
                rec(&r, 0);
            }
        }
    }

    #[test]
    fn truncado_e_corrompido_nao_causam_panico() {
        let data = sample();
        for len in (0..data.len()).step_by(7) {
            walk(&data[..len]);
        }
        // Corrompe a região das células byte a byte.
        for i in (BASE_BLOCK..data.len().min(BASE_BLOCK + 1200)).step_by(1) {
            for v in [0x00u8, 0xFF, 0x80] {
                let mut c = data.clone();
                c[i] = v;
                walk(&c);
            }
        }
    }

    #[test]
    fn corrupcao_em_lista_retorna_erro() {
        let data = sample();
        let root = Hive::new(&data).unwrap().root().unwrap();
        // Mantém a raiz válida mas aponta a lista de subchaves para o bloco base (assinatura inválida).
        let mut c = data.clone();
        let root_off = u32::from_le_bytes(c[0x24..0x28].try_into().unwrap()) as usize + BASE_BLOCK + 4;
        c[root_off + 28..root_off + 32].copy_from_slice(&0xFFFF_FF00u32.to_le_bytes());
        let _ = root;
        let broken = Hive::new(&c).unwrap().root().unwrap();
        assert!(broken.subkeys().is_err());
    }
}
