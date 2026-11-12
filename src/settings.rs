//! Armazenamento INI simples, compatível com o arquivo do QSettings do app Python.
//!
//! Linux: `$XDG_CONFIG_HOME/qcyctl/qcyctl.conf` (padrão `~/.config/qcyctl/qcyctl.conf`).
//! Windows: `%APPDATA%\qcyctl\qcyctl.conf`. Os valores ficam na seção `[General]`.
//! Chaves usadas: `classic_mac`, `ble_mac`, `reduce_motion`, `ignore_case`
//! (booleanos gravados como `true` e `false`).
//! Seções e chaves desconhecidas são preservadas ao gravar.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SECTION: &str = "General";

#[derive(Debug, Clone, Default)]
pub struct Settings {
    path: PathBuf,
    /// (seção, chave, valor já decodificado), na ordem do arquivo.
    entries: Vec<(String, String, String)>,
}

/// Caminho do arquivo de configuração conforme o sistema.
pub fn config_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData").join("Roaming"));
        return base.join("qcyctl").join("qcyctl.conf");
    }
    xdg_config_home().join("qcyctl").join("qcyctl.conf")
}

/// `$XDG_CONFIG_HOME`, ou `~/.config` quando ausente ou vazio.
pub fn xdg_config_home() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home().join(".config"),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Settings {
    /// Lê o arquivo padrão. Arquivo ausente ou ilegível dá um conjunto vazio.
    pub fn load() -> Self {
        Self::load_from(config_path())
    }

    pub fn load_from(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = fs::read(&path)
            .map(|b| parse(&String::from_utf8_lossy(&b)))
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|(s, k, _)| s == SECTION && k == key)
            .map(|(_, _, v)| v.clone())
    }

    /// Aceita `true`/`1`/`yes`/`on` e `false`/`0`/`no`/`off`; outro valor usa `default`.
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.get_str(key).map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("true" | "1" | "yes" | "on") => true,
            Some("false" | "0" | "no" | "off") => false,
            _ => default,
        }
    }

    /// Define o valor e grava o arquivo na hora (temp + rename).
    pub fn set(&mut self, key: &str, value: &str) -> io::Result<()> {
        match self
            .entries
            .iter_mut()
            .find(|(s, k, _)| s == SECTION && k == key)
        {
            Some(e) => e.2 = value.to_string(),
            None => self
                .entries
                .push((SECTION.into(), key.into(), value.into())),
        }
        self.save()
    }

    pub fn set_bool(&mut self, key: &str, value: bool) -> io::Result<()> {
        self.set(key, if value { "true" } else { "false" })
    }

    fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, serialize(&self.entries))?;
        fs::rename(&tmp, &self.path).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }
}

fn parse(text: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut section = String::from(SECTION);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let value = unquote(v.trim());
            let key = k.trim().to_string();
            match out
                .iter_mut()
                .find(|(s, kk, _): &&mut (String, String, String)| *s == section && *kk == key)
            {
                Some(e) => e.2 = value,
                None => out.push((section.clone(), key, value)),
            }
        }
    }
    out
}

fn unquote(v: &str) -> String {
    let Some(inner) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return v.to_string();
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn quote(v: &str) -> String {
    let precisa = v.starts_with(' ')
        || v.ends_with(' ')
        || v.chars()
            .any(|c| matches!(c, '"' | '\\' | ';' | ',' | '\n' | '\t' | '='));
    if !precisa {
        return v.to_string();
    }
    let mut out = String::from("\"");
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn serialize(entries: &[(String, String, String)]) -> String {
    let mut sections: Vec<&str> = Vec::new();
    for (s, _, _) in entries {
        if !sections.contains(&s.as_str()) {
            sections.push(s);
        }
    }
    if sections.is_empty() {
        sections.push(SECTION);
    }
    let mut out = String::new();
    for (i, sec) in sections.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("[{sec}]\n"));
        for (s, k, v) in entries {
            if s == sec {
                out.push_str(&format!("{k}={}\n", quote(v)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "melobuds-settings-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = temp_dir();
        let s = Settings::load_from(dir.join("nao/existe.conf"));
        assert_eq!(s.get_str("classic_mac"), None);
        assert!(s.get_bool("reduce_motion", true));
        assert!(!s.get_bool("reduce_motion", false));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_qsettings_file() {
        let dir = temp_dir();
        let path = dir.join("qcyctl.conf");
        fs::write(
            &path,
            "[General]\nclassic_mac=84:AC:60:11:22:33\nble_mac=AA:BB:CC:DD:EE:FF\nreduce_motion=true\nignore_case=false\n",
        )
        .unwrap();
        let s = Settings::load_from(&path);
        assert_eq!(s.get_str("classic_mac").as_deref(), Some("84:AC:60:11:22:33"));
        assert!(s.get_bool("reduce_motion", false));
        assert!(!s.get_bool("ignore_case", true));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn set_writes_atomically_and_roundtrips() {
        let dir = temp_dir();
        let path = dir.join("sub").join("qcyctl.conf");
        let mut s = Settings::load_from(&path);
        s.set("classic_mac", "84:AC:60:11:22:33").unwrap();
        s.set_bool("reduce_motion", true).unwrap();
        s.set_bool("reduce_motion", false).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "[General]\nclassic_mac=84:AC:60:11:22:33\nreduce_motion=false\n"
        );
        assert!(!dir.join("sub").join("qcyctl.conf.tmp").exists());
        let r = Settings::load_from(&path);
        assert_eq!(r.get_str("classic_mac").as_deref(), Some("84:AC:60:11:22:33"));
        assert!(!r.get_bool("reduce_motion", true));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn preserves_unknown_sections_and_keys() {
        let dir = temp_dir();
        let path = dir.join("qcyctl.conf");
        fs::write(&path, "[General]\nx=1\n\n[Outra]\ny=2\n").unwrap();
        let mut s = Settings::load_from(&path);
        s.set("ble_mac", "AA").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, "[General]\nx=1\nble_mac=AA\n\n[Outra]\ny=2\n");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn quoting_roundtrip() {
        let dir = temp_dir();
        let path = dir.join("qcyctl.conf");
        let mut s = Settings::load_from(&path);
        s.set("nome", "a;b,\"c\"").unwrap();
        assert_eq!(Settings::load_from(&path).get_str("nome").as_deref(), Some("a;b,\"c\""));
        let _ = fs::remove_dir_all(dir);
    }
}
