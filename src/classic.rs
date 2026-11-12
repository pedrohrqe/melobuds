//! Conexão de áudio (Bluetooth clássico) do fone pelo BlueZ, via D-Bus. Só Linux.
//!
//! O controle usa BLE e não exige pareamento. O áudio usa o endereço clássico, que precisa
//! estar pareado. Este módulo pareia (se preciso), marca como confiável e conecta.
//! No Windows o sistema reconecta o áudio sozinho; as funções retornam sem fazer nada.
//!
//! As funções são `async` e usam `dbus-tokio`: o chamador precisa estar dentro do runtime
//! tokio do app (o recurso da conexão D-Bus é criado com `tokio::spawn` na primeira chamada).
//! Há uma única conexão de sistema por processo, criada sob demanda e reaproveitada.

/// Estado do dispositivo clássico no BlueZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClassicStatus {
    pub known: bool,
    pub paired: bool,
    pub connected: bool,
}

/// Caminho do objeto D-Bus de um dispositivo no adaptador dado.
pub fn device_path(mac: &str, adapter: &str) -> String {
    format!(
        "/org/bluez/{adapter}/dev_{}",
        mac.to_uppercase().replace(':', "_")
    )
}

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(not(target_os = "linux"))]
pub use stub::*;

#[cfg(not(target_os = "linux"))]
mod stub {
    use super::ClassicStatus;

    /// Fora do Linux o estado do Bluetooth é desconhecido.
    pub async fn adapter_powered() -> Option<bool> {
        None
    }

    pub async fn set_powered(_on: bool) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("não suportado"))
    }

    pub async fn status(_mac: &str) -> ClassicStatus {
        ClassicStatus::default()
    }

    pub async fn disconnect(_mac: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    pub async fn ensure_connected(_mac: &str) -> String {
        "não suportado".to_string()
    }

    pub async fn reconnect(_mac: &str) -> bool {
        false
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{device_path, ClassicStatus};
    use dbus::arg::Variant;
    use dbus::nonblock::{Proxy, SyncConnection};
    use dbus::Error as DbusError;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::time::sleep;

    const BLUEZ: &str = "org.bluez";
    const DEVICE_IFACE: &str = "org.bluez.Device1";
    const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";
    const ADAPTER_PATH: &str = "/org/bluez/hci0";
    const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
    const A2DP_SINK: &str = "0000110b-0000-1000-8000-00805f9b34fb";
    /// Pair e Connect podem demorar (confirmação no fone).
    const CALL_TIMEOUT: Duration = Duration::from_secs(60);

    struct Shared {
        conn: Arc<SyncConnection>,
        alive: Arc<AtomicBool>,
    }

    static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

    /// Conexão com o barramento de sistema, reaproveitada entre as chamadas.
    ///
    /// Abrir uma conexão nova a cada consulta custa CPU (autenticação). Se o recurso da
    /// conexão terminou (barramento caiu), abre outra.
    async fn bus() -> Result<Arc<SyncConnection>, DbusError> {
        let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = guard.as_ref() {
            if s.alive.load(Ordering::Relaxed) {
                return Ok(s.conn.clone());
            }
        }
        let (resource, conn) = dbus_tokio::connection::new_system_sync()?;
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        tokio::spawn(async move {
            let err = resource.await;
            log::warn!("conexão D-Bus perdida: {err}");
            flag.store(false, Ordering::Relaxed);
        });
        *guard = Some(Shared { conn: conn.clone(), alive });
        Ok(conn)
    }

    type Prx = Proxy<'static, Arc<SyncConnection>>;

    fn proxy(conn: &Arc<SyncConnection>, path: String) -> Prx {
        Proxy::new(BLUEZ, path, CALL_TIMEOUT, conn.clone())
    }

    /// Lê uma propriedade sem introspecção. Retorna erro se o objeto não existe.
    async fn get_bool(p: &Prx, iface: &str, prop: &str) -> Result<bool, DbusError> {
        let (v,): (Variant<bool>,) = p.method_call(PROPS_IFACE, "Get", (iface, prop)).await?;
        Ok(v.0)
    }

    async fn set_bool(p: &Prx, iface: &str, prop: &str, value: bool) -> Result<(), DbusError> {
        p.method_call(PROPS_IFACE, "Set", (iface, prop, Variant(value))).await
    }

    /// Proxy do dispositivo, ou None se o BlueZ não conhece este MAC.
    ///
    /// A existência vem de uma leitura de `Connected`, sem introspecção.
    async fn device(conn: &Arc<SyncConnection>, mac: &str) -> Option<Prx> {
        let p = proxy(conn, device_path(mac, "hci0"));
        get_bool(&p, DEVICE_IFACE, "Connected").await.ok().map(|_| p)
    }

    fn error_text(e: &DbusError) -> String {
        match e.message() {
            Some(m) if !m.is_empty() => m.to_string(),
            _ => e.name().unwrap_or("").to_string(),
        }
    }

    pub async fn status(mac: &str) -> ClassicStatus {
        let Ok(conn) = bus().await else {
            return ClassicStatus::default();
        };
        let p = proxy(&conn, device_path(mac, "hci0"));
        let paired = get_bool(&p, DEVICE_IFACE, "Paired").await;
        let connected = get_bool(&p, DEVICE_IFACE, "Connected").await;
        match (paired, connected) {
            (Ok(paired), Ok(connected)) => ClassicStatus { known: true, paired, connected },
            _ => ClassicStatus::default(), // o BlueZ não conhece este MAC
        }
    }

    /// Desconecta um dispositivo, se o BlueZ o mostra conectado. Retorna `true` se desconectou.
    ///
    /// No endereço de áudio do HT05, um Connect() nosso logo depois falha (ver `reconnect`).
    /// Por isso o uso no áudio é só com os fones na caixa: a conexão seguinte parte do fone.
    pub async fn disconnect(mac: &str) -> anyhow::Result<bool> {
        let conn = bus().await?;
        let Some(dev) = device(&conn, mac).await else {
            return Ok(false);
        };
        if !get_bool(&dev, DEVICE_IFACE, "Connected").await? {
            return Ok(false);
        }
        dev.method_call::<(), _, _, _>(DEVICE_IFACE, "Disconnect", ()).await?;
        Ok(true)
    }

    /// Estado do Bluetooth do sistema. `None` = desconhecido (sem D-Bus).
    pub async fn adapter_powered() -> Option<bool> {
        // Sem barramento de sistema: desconhecido.
        let conn = bus().await.ok()?;
        let p = proxy(&conn, ADAPTER_PATH.to_string());
        // Sem adaptador, ou o serviço Bluetooth parado: desligado.
        Some(get_bool(&p, ADAPTER_IFACE, "Powered").await.unwrap_or(false))
    }

    /// Liga ou desliga o Bluetooth do sistema. Retorna erro se o sistema recusa.
    pub async fn set_powered(on: bool) -> anyhow::Result<()> {
        let conn = bus().await?;
        let p = proxy(&conn, ADAPTER_PATH.to_string());
        set_bool(&p, ADAPTER_IFACE, "Powered", on).await?;
        Ok(())
    }

    /// Conecta primeiro o perfil de música (A2DP) e depois os outros perfis.
    ///
    /// Um Connect() direto abre o perfil de chamadas (HFP) antes: o sistema escolhe o modo
    /// "headset", de baixa qualidade, e só troca para o modo normal segundos depois.
    async fn connect_audio(d: &Prx) -> Result<(), DbusError> {
        if let Err(exc) = d
            .method_call::<(), _, _, _>(DEVICE_IFACE, "ConnectProfile", (A2DP_SINK,))
            .await
        {
            log::info!("A2DP primeiro falhou ({exc}); conexão padrão");
            return d.method_call(DEVICE_IFACE, "Connect", ()).await;
        }
        // Perfis restantes (chamadas, controle de mídia).
        if let Err(exc) = d.method_call::<(), _, _, _>(DEVICE_IFACE, "Connect", ()).await {
            log::info!("perfis restantes: {exc}");
        }
        Ok(())
    }

    /// Pareia (se preciso), confia e conecta o áudio. Retorna uma descrição do resultado.
    pub async fn ensure_connected(mac: &str) -> String {
        let conn = match bus().await {
            Ok(c) => c,
            Err(e) => return format!("falha ao acessar o D-Bus: {}", error_text(&e)),
        };
        let Some(d) = device(&conn, mac).await else {
            return "o BlueZ não conhece o fone. Coloque o fone em modo de pareamento.".into();
        };
        if get_bool(&d, DEVICE_IFACE, "Connected").await.unwrap_or(false) {
            return "conectado".into();
        }
        let attempt = async {
            if !get_bool(&d, DEVICE_IFACE, "Paired").await? {
                log::info!("pareando {mac}");
                d.method_call::<(), _, _, _>(DEVICE_IFACE, "Pair", ()).await?;
            }
            set_bool(&d, DEVICE_IFACE, "Trusted", true).await?;
            connect_audio(&d).await
        }
        .await;
        if let Err(exc) = attempt {
            log::info!("falha no áudio {mac}: {exc}");
            // A falha pode ser uma colisão com a conexão que o próprio fone iniciou.
            // Espera essa conexão terminar antes de declarar o erro.
            for _ in 0..3 {
                sleep(Duration::from_secs(1)).await;
                if get_bool(&d, DEVICE_IFACE, "Connected").await.unwrap_or(false) {
                    return "conectado".into();
                }
            }
            let name = exc.name().unwrap_or("");
            if name.contains("AuthenticationFailed") || name.contains("AuthenticationRejected") {
                return "o fone recusou o pareamento. Coloque o fone em modo de pareamento.".into();
            }
            return format!("falha ao conectar o áudio: {}", error_text(&exc));
        }
        "conectado".into()
    }

    /// Reinicia o adaptador e reconecta o áudio. O HT05 volta a anunciar BLE por cerca de 60 s.
    ///
    /// Um Device1.Disconnect() seguido de Connect() não serve: com o HT05, o BlueZ fica preso em
    /// "operation already in progress" e recusa toda conexão até o adaptador reiniciar.
    /// Por isso o ciclo do adaptador faz a desconexão.
    ///
    /// Retorna `false` se o D-Bus ou o adaptador recusa, ou se o fone não reconecta.
    pub async fn reconnect(mac: &str) -> bool {
        let conn = match bus().await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("reconectar {mac}: sem D-Bus: {e}");
                return false;
            }
        };
        let adapter = proxy(&conn, ADAPTER_PATH.to_string());
        let off = set_bool(&adapter, ADAPTER_IFACE, "Powered", false).await;
        if off.is_ok() {
            sleep(Duration::from_secs(2)).await;
        }
        // Religa sempre, mesmo se o desligamento falhou.
        let on = set_bool(&adapter, ADAPTER_IFACE, "Powered", true).await;
        if let Err(e) = off.and(on) {
            log::warn!("reconectar {mac}: ciclo do adaptador falhou: {e}");
            return false;
        }
        sleep(Duration::from_secs(3)).await;
        for tentativa in 1..=4 {
            let Some(d) = device(&conn, mac).await else {
                return false;
            };
            let result = async {
                if !get_bool(&d, DEVICE_IFACE, "Connected").await? {
                    connect_audio(&d).await?;
                }
                Ok::<(), DbusError>(())
            }
            .await;
            match result {
                Ok(()) => return true,
                Err(exc) => {
                    log::info!("falha ao reconectar {mac} (tentativa {tentativa}): {exc}");
                    sleep(Duration::from_secs(3)).await;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_from_mac() {
        assert_eq!(
            device_path("84:ac:60:11:22:33", "hci0"),
            "/org/bluez/hci0/dev_84_AC_60_11_22_33"
        );
        assert_eq!(
            device_path("84:AC:60:11:22:33", "hci1"),
            "/org/bluez/hci1/dev_84_AC_60_11_22_33"
        );
    }

    #[test]
    fn status_default_is_unknown() {
        let s = ClassicStatus::default();
        assert!(!s.known && !s.paired && !s.connected);
    }
}
