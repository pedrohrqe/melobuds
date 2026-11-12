//! Inicialização do app com o sistema.
//!
//! Linux: arquivo .desktop em `~/.config/autostart`. Windows: chave Run do usuário no registro.
//! O app inicia com `--hidden` (na bandeja, quando o sistema tem bandeja).

use std::io;
use std::path::PathBuf;

pub const APP_NAME: &str = "MeloBuds by @pedrohrqe";
/// Nome do arquivo .desktop e do valor no registro.
pub const ENTRY: &str = "melobuds";
#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Comando que abre este app: o AppImage (`$APPIMAGE`) ou o executável atual, mais `--hidden`.
pub fn command() -> Vec<String> {
    let exe = std::env::var("APPIMAGE")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from(ENTRY));
    vec![exe.to_string_lossy().into_owned(), "--hidden".to_string()]
}

fn desktop_file() -> PathBuf {
    crate::settings::xdg_config_home()
        .join("autostart")
        .join(format!("{ENTRY}.desktop"))
}

/// Escapa um argumento para a chave `Exec` de um arquivo .desktop.
fn quote_exec(arg: &str) -> String {
    let seguro = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c));
    if seguro {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn desktop_contents(cmd: &[String]) -> String {
    let exec = cmd.iter().map(|a| quote_exec(a)).collect::<Vec<_>>().join(" ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={APP_NAME}\n\
         Comment=Inicia o controle do fone na bandeja\n\
         Exec={exec}\n\
         Icon=qcyctl\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// Equivalente a `subprocess.list2cmdline` (valor do registro no Windows).
#[cfg_attr(not(windows), allow(dead_code))]
fn cmdline(cmd: &[String]) -> String {
    cmd.iter()
        .map(|a| {
            if a.is_empty() || a.contains([' ', '\t', '"']) {
                format!("\"{}\"", a.replace('"', "\\\""))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(ENTRY))
        .is_ok()
}

/// Liga ou desliga a inicialização com o sistema. Retorna erro se o sistema recusa.
#[cfg(windows)]
pub fn set_enabled(on: bool) -> io::Result<()> {
    use winreg::{enums::{HKEY_CURRENT_USER, KEY_SET_VALUE}, RegKey};
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    if on {
        key.set_value(ENTRY, &cmdline(&command()))
    } else {
        match key.delete_value(ENTRY) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    desktop_file().is_file()
}

/// Liga ou desliga a inicialização com o sistema. Retorna erro se o sistema recusa.
#[cfg(not(windows))]
pub fn set_enabled(on: bool) -> io::Result<()> {
    let path = desktop_file();
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, desktop_contents(&command()))
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Os testes mudam variáveis de ambiente: um de cada vez.
    static ENV: Mutex<()> = Mutex::new(());

    #[test]
    fn enable_and_disable_in_temp_dir() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("melobuds-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("APPIMAGE");

        assert!(!is_enabled());
        set_enabled(true).unwrap();
        assert!(is_enabled());
        let path = dir.join("autostart").join("melobuds.desktop");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(text.contains("Name=MeloBuds by @pedrohrqe\n"));
        let exec = text.lines().find(|l| l.starts_with("Exec=")).unwrap();
        assert!(exec.ends_with(" --hidden"), "{exec}");
        set_enabled(false).unwrap();
        assert!(!is_enabled());
        set_enabled(false).unwrap(); // desligar de novo não falha

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn appimage_overrides_current_exe() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("APPIMAGE", "/opt/Meu App/melobuds.AppImage");
        let cmd = command();
        std::env::remove_var("APPIMAGE");
        assert_eq!(cmd, vec!["/opt/Meu App/melobuds.AppImage", "--hidden"]);
        let desktop = desktop_contents(&cmd);
        assert!(desktop.contains("Exec=\"/opt/Meu App/melobuds.AppImage\" --hidden\n"));
    }

    #[test]
    fn cmdline_quotes_spaces() {
        let cmd = vec![r"C:\Program Files\MeloBuds\melobuds.exe".to_string(), "--hidden".into()];
        assert_eq!(cmdline(&cmd), r#""C:\Program Files\MeloBuds\melobuds.exe" --hidden"#);
    }
}
