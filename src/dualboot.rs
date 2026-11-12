//! Sincroniza a chave de pareamento Bluetooth clássico do Windows para o BlueZ (Linux).
//!
//! No dual-boot, Windows e Linux usam o mesmo adaptador. Cada pareamento gera uma chave nova,
//! por isso o fone deixa de conectar no sistema que pareou antes. Este módulo copia a chave
//! que o Windows guardou no registro para o arquivo `info` do BlueZ.
//!
//! Procedimento:
//! 1. Parear o fone no Linux (uma vez, para criar o diretório do dispositivo no BlueZ).
//! 2. Iniciar o Windows e parear o fone lá.
//! 3. Voltar ao Linux e executar `sudo qcyctl dualboot`.
//!
//! Só o registro do Windows é lido. Nada é escrito na partição do Windows.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Diretório onde o BlueZ guarda os dispositivos pareados.
pub const BLUEZ_DIR: &str = "/var/lib/bluetooth";
/// Caminho do hive SYSTEM dentro da partição Windows.
pub const HIVE_REL: &str = "Windows/System32/config/SYSTEM";
const KEYS_PATH: &str = "Services\\BTHPORT\\Parameters\\Keys";

#[derive(Debug)]
pub struct DualbootError(pub String);

impl fmt::Display for DualbootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DualbootError {}

type Result<T> = std::result::Result<T, DualbootError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(DualbootError(msg.into()))
}

/// Chaves do Windows: {adaptador: {dispositivo: chave de 16 bytes}}, MACs no formato AA:BB:...
pub type WinKeys = BTreeMap<String, BTreeMap<String, [u8; 16]>>;

/// "AA:BB:CC:DD:EE:FF" -> "aabbccddeeff".
pub fn mac_hex(mac: &str) -> String {
    mac.replace(':', "").to_lowercase()
}

/// "aabbccddeeff" -> "AA:BB:CC:DD:EE:FF". Usa só os 12 primeiros caracteres.
pub fn mac_colon(hexstr: &str) -> String {
    let chars: Vec<char> = hexstr.chars().take(12).collect();
    chars
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(":")
        .to_uppercase()
}

/// Chave de 16 bytes em hexadecimal maiúsculo, como o BlueZ guarda.
pub fn key_hex(key: &[u8]) -> String {
    key.iter().map(|b| format!("{b:02X}")).collect()
}

/// Resultado da sincronização de um dispositivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResult {
    pub adapter: String,
    pub device: String,
    pub old_key: Option<String>,
    pub new_key: String,
    pub changed: bool,
}

/// Lê a chave atual (`Key` na seção `[LinkKey]`) do conteúdo de um arquivo `info`.
pub fn read_link_key(info: &str) -> Option<String> {
    let mut in_section = false;
    for line in info.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            in_section = &t[1..t.len() - 1] == "LinkKey";
        } else if in_section {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == "Key" {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Troca a chave no conteúdo de um arquivo `info`, sem tocar nas outras linhas.
///
/// Sem a seção `[LinkKey]`, cria a seção com `Type=4` e `PINLength=0`, como o Python.
pub fn set_link_key(info: &str, new_key: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut has_section = false;
    let mut done = false;
    for line in info.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            if in_section && !done {
                out.push(format!("Key={new_key}"));
                done = true;
            }
            in_section = &t[1..t.len() - 1] == "LinkKey";
            has_section |= in_section;
            out.push(line.to_string());
            continue;
        }
        if in_section && !done {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == "Key" {
                    out.push(format!("Key={new_key}"));
                    done = true;
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if in_section && !done {
        out.push(format!("Key={new_key}"));
        done = true;
    }
    if !has_section {
        if out.last().is_some_and(|l| !l.is_empty()) {
            out.push(String::new());
        }
        out.extend(["[LinkKey]".to_string(), "Type=4".to_string(), "PINLength=0".to_string(), format!("Key={new_key}")]);
    }
    debug_assert!(done || !has_section);
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// Copia as chaves para o BlueZ em `bluez_dir`. Sem `device`, copia todas que existem nos dois.
///
/// Antes de alterar um `info`, grava o backup `info.bak-<timestamp>` ao lado dele.
pub fn sync_with(bluez_dir: &Path, win: &WinKeys, device: Option<&str>, dry_run: bool) -> Result<Vec<SyncResult>> {
    let mut results = Vec::new();
    for (adapter, devices) in win {
        let adapter_dir = bluez_dir.join(adapter);
        if !adapter_dir.is_dir() {
            continue;
        }
        for (dev_mac, key) in devices {
            if let Some(d) = device {
                if *dev_mac != d.to_uppercase() {
                    continue;
                }
            }
            let info = adapter_dir.join(dev_mac).join("info");
            if !info.is_file() {
                continue; // dispositivo nunca pareado no Linux
            }
            let text = std::fs::read_to_string(&info).map_err(|e| DualbootError(format!("falha ao ler {}: {e}", info.display())))?;
            let new = key_hex(key);
            let old = read_link_key(&text);
            let changed = old.as_deref() != Some(new.as_str());
            results.push(SyncResult { adapter: adapter.clone(), device: dev_mac.clone(), old_key: old, new_key: new.clone(), changed });
            if changed && !dry_run {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = info.with_file_name(format!("info.bak-{ts}"));
                std::fs::copy(&info, &backup)
                    .map_err(|e| DualbootError(format!("falha ao criar o backup {}: {e}", backup.display())))?;
                std::fs::write(&info, set_link_key(&text, &new))
                    .map_err(|e| DualbootError(format!("falha ao gravar {}: {e}", info.display())))?;
            }
        }
    }
    Ok(results)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::regf::{Hive, REG_BINARY};

    /// Acha uma partição Windows montada (procura o hive SYSTEM).
    pub fn find_windows_root(explicit: Option<&str>) -> Result<PathBuf> {
        if let Some(e) = explicit {
            let root = PathBuf::from(e);
            if !root.join(HIVE_REL).is_file() {
                return err(format!("hive SYSTEM não encontrado em {}", root.join(HIVE_REL).display()));
            }
            return Ok(root);
        }
        let mounts = std::fs::read_to_string("/proc/mounts").map_err(|e| DualbootError(format!("falha ao ler /proc/mounts: {e}")))?;
        for line in mounts.lines() {
            let Some(mnt) = line.split_whitespace().nth(1) else { continue };
            let mnt = mnt.replace("\\040", " ");
            // Pontos de montagem sem permissão (/sys, /proc...) falham em is_file e são ignorados.
            if Path::new(&mnt).join(HIVE_REL).is_file() {
                return Ok(PathBuf::from(mnt));
            }
        }
        err("nenhuma partição Windows montada. Monte em modo somente leitura, por exemplo:\n  udisksctl mount -b /dev/nvme0n1p3 -o ro\ne execute de novo (ou use --win <ponto de montagem>).")
    }

    fn hive_err(e: impl fmt::Display) -> DualbootError {
        DualbootError(format!("hive SYSTEM inválido: {e}"))
    }

    /// Lê as chaves do hive SYSTEM já carregado em memória.
    pub fn read_keys_from_hive(bytes: &[u8]) -> Result<WinKeys> {
        let hive = Hive::new(bytes).map_err(hive_err)?;
        let root = hive.root().map_err(hive_err)?;
        let current = root
            .subpath("Select")
            .ok()
            .flatten()
            .and_then(|sel| sel.value("Current").ok().flatten())
            .and_then(|v| v.dword())
            .unwrap_or(1);
        let path = format!("ControlSet{current:03}\\{KEYS_PATH}");
        let keys = match root.subpath(&path) {
            Ok(Some(k)) => k,
            _ => return err("o Windows não tem chaves Bluetooth registradas"),
        };
        let mut out = WinKeys::new();
        for adapter in keys.subkeys().map_err(hive_err)? {
            let mut devices = BTreeMap::new();
            for value in adapter.values().map_err(hive_err)? {
                // Chaves de BR/EDR são REG_BINARY de 16 bytes com o MAC como nome.
                if value.data_type != REG_BINARY || value.name.chars().count() != 12 {
                    continue;
                }
                if let Ok(key) = <[u8; 16]>::try_from(value.data.as_slice()) {
                    devices.insert(mac_colon(&value.name), key);
                }
            }
            if !devices.is_empty() {
                out.insert(mac_colon(adapter.name()), devices);
            }
        }
        Ok(out)
    }

    /// Retorna {adaptador: {dispositivo: chave de 16 bytes}} com MACs no formato AA:BB:...
    pub fn read_windows_keys(windows_root: &Path) -> Result<WinKeys> {
        let path = windows_root.join(HIVE_REL);
        let bytes = std::fs::read(&path).map_err(|e| DualbootError(format!("falha ao ler {}: {e}", path.display())))?;
        read_keys_from_hive(&bytes)
    }

    /// UID efetivo do processo (lido de /proc, sem dependência extra).
    fn effective_uid() -> Option<u32> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("Uid:"))?;
        line.split_whitespace().nth(2)?.parse().ok()
    }

    pub fn sync(windows_root: &Path, device: Option<&str>, dry_run: bool) -> Result<Vec<SyncResult>> {
        let win = read_windows_keys(windows_root)?;
        sync_with(Path::new(BLUEZ_DIR), &win, device, dry_run)
    }

    pub fn restart_bluetooth() -> Result<()> {
        let ok = std::process::Command::new("systemctl")
            .args(["restart", "bluetooth"])
            .status()
            .map_err(|e| DualbootError(format!("falha ao executar systemctl: {e}")))?
            .success();
        if ok { Ok(()) } else { err("systemctl restart bluetooth falhou") }
    }

    pub fn run(win: Option<&str>, device: Option<&str>, dry_run: bool) -> Result<i32> {
        let root = find_windows_root(win)?;
        if effective_uid() != Some(0) {
            return err("execute com sudo (o BlueZ guarda as chaves em /var/lib/bluetooth)");
        }
        let results = sync(&root, device, dry_run)?;
        if results.is_empty() {
            println!("Nenhum dispositivo pareado nos dois sistemas.");
            return Ok(1);
        }
        for r in &results {
            let state = if r.changed && !dry_run {
                "atualizada"
            } else if r.changed {
                "diferente"
            } else {
                "igual"
            };
            println!("{} (adaptador {}): chave {state}", r.device, r.adapter);
        }
        if results.iter().any(|r| r.changed) && !dry_run {
            restart_bluetooth()?;
            println!("Serviço bluetooth reiniciado. Conecte o fone normalmente.");
        }
        Ok(0)
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

/// Fora do Linux o comando não existe.
#[cfg(not(target_os = "linux"))]
pub fn run(_win: Option<&str>, _device: Option<&str>, _dry_run: bool) -> Result<i32> {
    err("este comando roda só no Linux")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formata_mac() {
        assert_eq!(mac_hex("AA:BB:CC:DD:EE:FF"), "aabbccddeeff");
        assert_eq!(mac_colon("aabbccddeeff"), "AA:BB:CC:DD:EE:FF");
        assert_eq!(mac_colon(&mac_hex("01:23:45:67:89:AB")), "01:23:45:67:89:AB");
    }

    #[test]
    fn formata_chave() {
        let mut k = [0u8; 16];
        k[0] = 0xAB;
        k[15] = 0x01;
        assert_eq!(key_hex(&k), "AB000000000000000000000000000001");
    }

    const INFO: &str = "[General]\nName=QCY HT05\nTrusted=true\n\n[LinkKey]\nKey=00112233445566778899AABBCCDDEEFF\nType=4\nPINLength=0\n\n[DeviceID]\nSource=1\n";

    #[test]
    fn le_chave_atual() {
        assert_eq!(read_link_key(INFO).as_deref(), Some("00112233445566778899AABBCCDDEEFF"));
        assert_eq!(read_link_key("[General]\nName=x\n"), None);
    }

    #[test]
    fn troca_chave_preservando_o_resto() {
        let out = set_link_key(INFO, "FFEEDDCCBBAA99887766554433221100");
        assert_eq!(read_link_key(&out).as_deref(), Some("FFEEDDCCBBAA99887766554433221100"));
        assert!(out.contains("Name=QCY HT05") && out.contains("Source=1") && out.contains("Type=4"));
        assert_eq!(out.matches("Key=").count(), 1);
    }

    #[test]
    fn cria_secao_sem_linkkey() {
        let out = set_link_key("[General]\nName=x\n", "AA");
        assert!(out.ends_with("[LinkKey]\nType=4\nPINLength=0\nKey=AA\n"));
        assert_eq!(read_link_key(&out).as_deref(), Some("AA"));
    }

    #[test]
    fn adiciona_key_em_secao_sem_key() {
        let out = set_link_key("[LinkKey]\nType=4\n[Other]\nA=1\n", "BB");
        assert_eq!(read_link_key(&out).as_deref(), Some("BB"));
        assert!(out.contains("[Other]\nA=1"));
    }

    // O formato de pastas do BlueZ usa ':' no nome, o que o Windows não aceita.
    #[cfg(unix)]
    fn fixture() -> (PathBuf, WinKeys) {
        let dir = std::env::temp_dir().join(format!("qcyctl-dualboot-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dev = dir.join("11:22:33:44:55:66").join("AA:BB:CC:DD:EE:FF");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("info"), INFO).unwrap();
        let mut devices = BTreeMap::new();
        devices.insert("AA:BB:CC:DD:EE:FF".to_string(), [0x11u8; 16]);
        devices.insert("00:00:00:00:00:01".to_string(), [0x22u8; 16]); // sem info no Linux
        let mut win = WinKeys::new();
        win.insert("11:22:33:44:55:66".to_string(), devices);
        win.insert("99:99:99:99:99:99".to_string(), BTreeMap::new()); // adaptador ausente no BlueZ
        (dir, win)
    }

    #[test]
    #[cfg(unix)]
    fn sync_dry_run_nao_altera() {
        let (dir, win) = fixture();
        let r = sync_with(&dir, &win, None, true).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].changed);
        let info = dir.join("11:22:33:44:55:66/AA:BB:CC:DD:EE:FF/info");
        assert_eq!(std::fs::read_to_string(info).unwrap(), INFO);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn sync_grava_com_backup_e_depois_fica_igual() {
        let (dir, win) = fixture();
        let r = sync_with(&dir, &win, Some("aa:bb:cc:dd:ee:ff"), false).unwrap();
        assert!(r[0].changed);
        let devdir = dir.join("11:22:33:44:55:66/AA:BB:CC:DD:EE:FF");
        let text = std::fs::read_to_string(devdir.join("info")).unwrap();
        assert_eq!(read_link_key(&text).unwrap(), "11".repeat(16));
        let backups: Vec<_> = std::fs::read_dir(&devdir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("info.bak-")).collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), INFO);
        let again = sync_with(&dir, &win, None, false).unwrap();
        assert!(!again[0].changed);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[cfg(unix)]
    fn sync_filtra_dispositivo() {
        let (dir, win) = fixture();
        assert!(sync_with(&dir, &win, Some("00:00:00:00:00:02"), false).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
