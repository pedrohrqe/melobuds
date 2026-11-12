//! Mantém a conexão BLE com o fone e publica o estado como eventos.
//!
//! O HT05 só anuncia BLE por cerca de 60 s depois de conectar o áudio. Por isso o
//! scan roda sem parar enquanto o fone estiver desconectado, e a conexão fica aberta.
//! Os fatos de hardware estão em `docs/` e nos comentários de `classic.rs`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use btleplug::platform::Adapter;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::classic;
use crate::device::{self, BluetoothOff, DeviceState, HT05};
use crate::protocol as p;
use crate::settings::Settings;

/// Tempo em que a tela mantém o estado depois de uma queda do BLE.
const HOLD: Duration = Duration::from_secs(20);
/// Pausa curta: evita pedir o áudio no mesmo instante em que o fone chama o computador.
const AUDIO_GRACE: Duration = Duration::from_millis(1500);
/// Intervalo da sonda do elo de controle.
const PROBE_IDLE: Duration = Duration::from_secs(15);
/// Espera pela resposta da sonda.
const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
const SUPPORTED: bool = cfg!(target_os = "linux");

#[derive(Debug, Clone)]
pub enum Event {
    State(DeviceState),
    Status(String),
    Error(String),
    /// Bluetooth do sistema ligado ou desligado.
    Bluetooth(bool),
    /// Áudio conectado, mas o controle BLE não está visível.
    AudioOnly(bool),
}

#[derive(Debug, Clone)]
pub enum Command {
    Noise(p::NoiseMode),
    Eq(p::Equalizer),
    Key(u8, u8),
    Game(bool),
    Sleep(bool),
    Rename(String),
    ConnectAudio,
    Wake,
    EnableBluetooth,
}

pub type Emit = Arc<dyn Fn(Event) + Send + Sync>;
pub type SharedSettings = Arc<Mutex<Settings>>;

/// Os dois fones estão na caixa: cada um carrega ou não aparece, e ao menos um carrega.
pub fn in_case(battery: Option<&p::Battery>) -> bool {
    let Some(b) = battery else { return false };
    let buds = [b.left, b.right];
    buds.iter().any(|x| x.charging) && buds.iter().all(|x| x.charging || x.percent.unwrap_or(0) == 0)
}

struct Inner {
    device: Option<Arc<HT05>>,
    /// MAC clássico salvo: permite reabrir o anúncio BLE quando o áudio já está conectado.
    classic_mac: Option<String>,
    next_audio_check: Instant,
    audio_fails: u32,
    audio_only: bool,
    audio_seen: bool,
    bluetooth_on: bool,
    probe_due: Instant,
    probe_sent: Option<Instant>,
    waking: bool,
    /// Instante da queda do BLE.
    lost_at: Option<Instant>,
}

#[derive(Clone)]
pub struct Controller {
    inner: Rc<RefCell<Inner>>,
    emit: Emit,
    settings: SharedSettings,
    address: Option<String>,
}

impl Controller {
    pub fn new(address: Option<String>, settings: SharedSettings, emit: Emit) -> Self {
        let classic_mac = settings.lock().unwrap().get_str("classic_mac").filter(|s| !s.is_empty());
        let now = Instant::now();
        Self {
            inner: Rc::new(RefCell::new(Inner {
                device: None,
                classic_mac,
                next_audio_check: now,
                audio_fails: 0,
                audio_only: false,
                audio_seen: false,
                bluetooth_on: true,
                probe_due: now,
                probe_sent: None,
                waking: false,
                lost_at: None,
            })),
            emit,
            settings,
            address,
        }
    }

    fn status(&self, s: &str) {
        (self.emit)(Event::Status(s.into()));
    }
    fn error(&self, s: &str) {
        (self.emit)(Event::Error(s.into()));
    }
    fn state(&self, s: DeviceState) {
        (self.emit)(Event::State(s));
    }

    fn connected(&self) -> bool {
        self.inner.borrow().device.as_ref().is_some_and(|d| d.is_connected())
    }

    fn classic_mac(&self) -> Option<String> {
        self.inner.borrow().classic_mac.clone()
    }

    /// Executa o laço de conexão e os comandos. Chame dentro de um `LocalSet`.
    pub async fn run(self, mut rx: mpsc::UnboundedReceiver<Command>) {
        let me = self.clone();
        tokio::task::spawn_local(async move {
            while let Some(cmd) = rx.recv().await {
                me.command(cmd);
            }
        });
        self.main_loop().await;
    }

    async fn main_loop(&self) {
        let mut adapter: Option<Adapter> = None;
        loop {
            if self.inner.borrow().waking {
                // O adaptador reinicia; o scan volta depois.
                sleep(Duration::from_millis(500)).await;
                continue;
            }
            // Bluetooth do sistema desligado: o app mostra o aviso e não tenta mais nada.
            if !self.connected() && classic::adapter_powered().await == Some(false) {
                self.bluetooth_off();
                sleep(Duration::from_millis(1500)).await;
                continue;
            }
            if self.connected() {
                self.check_link().await;
                self.check_audio(false).await;
                sleep(Duration::from_secs(1)).await;
                continue;
            }
            let ad = match &adapter {
                Some(a) => a.clone(),
                None => match device::adapter().await {
                    Ok(a) => {
                        adapter = Some(a.clone());
                        a
                    }
                    Err(e) => {
                        log::info!("sem adaptador: {e}");
                        self.bluetooth_off();
                        sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                },
            };

            // Queda curta do BLE: mantém a tela com o último estado e reconecta em silêncio.
            let now = Instant::now();
            let lost = {
                let i = self.inner.borrow();
                i.device.clone().filter(|_| i.lost_at.is_none())
            };
            if let Some(dev) = lost {
                self.inner.borrow_mut().lost_at = Some(now);
                log::info!("controle BLE caiu; reconectando");
                if in_case(dev.state().battery.as_ref()) {
                    // Na caixa, o fone solta o controle, mas o sistema continua a mostrar
                    // o áudio como conectado (o adaptador não percebe a queda). O app
                    // encerra essa conexão para o sistema mostrar o estado real.
                    log::info!("fones na caixa; o app encerra a conexão de áudio no sistema");
                    self.inner.borrow_mut().lost_at = now.checked_sub(HOLD);
                    if let Some(mac) = self.classic_mac() {
                        if let Err(e) = classic::disconnect(&mac).await {
                            log::info!("falha ao encerrar o áudio: {e}");
                        }
                    }
                }
            }
            let lost_at = self.inner.borrow().lost_at;
            let mut holding = lost_at.is_some_and(|t| now.duration_since(t) < HOLD);
            // A espera só esconde quedas curtas do controle. Sem o áudio, o fone saiu de
            // verdade (caixa, desligado, desconectado no sistema): a tela mostra isso logo.
            if holding && !self.audio_up().await {
                holding = false;
            }
            if holding {
                self.status("Reconectando o controle…");
            } else {
                {
                    let mut i = self.inner.borrow_mut();
                    i.device = None;
                    i.lost_at = None;
                }
                self.status("Procurando o fone…");
                self.state(DeviceState::default());
            }

            // O sistema mantém o elo BLE aberto depois que o app fecha. Esse elo não
            // responde mais, e com ele aberto o fone não anuncia. O app o derruba (o
            // áudio não é afetado); o fone volta a anunciar e o scan o encontra.
            if self.drop_stale_ble().await {
                sleep(Duration::from_millis(500)).await;
            }
            let timeout = Duration::from_secs(if holding { 3 } else { 6 });
            let found = match device::find_first_with_adv(&ad, timeout).await {
                Ok(f) => f,
                Err(e) if e.is::<BluetoothOff>() => {
                    // Windows e outros sistemas: o scan é quem informa o Bluetooth desligado.
                    self.bluetooth_off();
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                Err(e) => {
                    log::info!("falha no scan: {e}");
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };
            self.set_bluetooth(true); // o scan funcionou
            let Some(found) = found else {
                if !holding {
                    self.check_audio_only().await;
                }
                continue;
            };
            if let Some(a) = &self.address {
                if !found.address.eq_ignore_ascii_case(a) {
                    continue;
                }
            }
            let mac = found.advertisement.classic_mac.clone();
            self.inner.borrow_mut().classic_mac = Some(mac.clone());
            let _ = self.settings.lock().unwrap().set("classic_mac", &mac);
            self.set_audio_only(false);
            if !holding {
                self.status("Conectando…");
            }
            let res = tokio::time::timeout(
                Duration::from_secs(20),
                HT05::connect(&ad, found.peripheral, Duration::from_secs(8)),
            )
            .await;
            let dev = match res {
                Ok(Ok(d)) => d,
                Ok(Err(e)) => {
                    self.connect_failed(holding, &e.to_string()).await;
                    continue;
                }
                Err(_) => {
                    self.connect_failed(holding, "tempo esgotado").await;
                    continue;
                }
            };
            // Só publica o estado depois da leitura inicial, para a tela não piscar.
            // A queda é tratada por este laço, não pelo estado do fone.
            let emit = self.emit.clone();
            dev.on_change(Arc::new(move |s: DeviceState| {
                if s.connected {
                    emit(Event::State(s));
                }
            }));
            {
                let mut i = self.inner.borrow_mut();
                i.device = Some(dev.clone());
                i.lost_at = None;
                i.probe_sent = None;
                i.probe_due = Instant::now() + PROBE_IDLE;
                // Ao sair da caixa, o fone chama o computador para o áudio por conta própria.
                // Um Connect() nosso no mesmo instante colide com essa chamada e as duas falham
                // ("Operation already in progress"). Por isso a primeira verificação espera.
                i.audio_fails = 0;
                i.audio_seen = false;
                i.next_audio_check = Instant::now() + AUDIO_GRACE;
            }
            let _ = self.settings.lock().unwrap().set("ble_mac", &found.address);
            self.state(dev.state());
            self.status("Conectado");
        }
    }

    async fn connect_failed(&self, holding: bool, err: &str) {
        log::info!("falha de conexão: {err}");
        if holding {
            sleep(Duration::from_secs(1)).await;
            return;
        }
        self.status("Fone não encontrado. Tire da caixa ou reconecte o áudio.");
        sleep(Duration::from_secs(3)).await;
    }

    fn set_bluetooth(&self, on: bool) {
        let changed = {
            let mut i = self.inner.borrow_mut();
            let c = i.bluetooth_on != on;
            i.bluetooth_on = on;
            c
        };
        if changed {
            log::info!("Bluetooth do sistema {}", if on { "ligado" } else { "desligado" });
            (self.emit)(Event::Bluetooth(on));
        }
    }

    /// Limpa a tela uma vez; depois só espera o Bluetooth voltar.
    fn bluetooth_off(&self) {
        if !self.inner.borrow().bluetooth_on {
            return;
        }
        self.set_bluetooth(false);
        {
            let mut i = self.inner.borrow_mut();
            i.device = None;
            i.lost_at = None;
        }
        self.set_audio_only(false);
        self.state(DeviceState::default());
        self.status("Bluetooth desligado");
    }

    /// Derruba o elo BLE que o sistema manteve aberto. Retorna true se havia um.
    async fn drop_stale_ble(&self) -> bool {
        if !SUPPORTED {
            return false;
        }
        let mac = self.address.clone().or_else(|| self.settings.lock().unwrap().get_str("ble_mac"));
        let Some(mac) = mac.filter(|m| !m.is_empty()) else { return false };
        if !classic::status(&mac).await.connected {
            return false;
        }
        classic::disconnect(&mac).await.unwrap_or(false)
    }

    /// Detecta o elo BLE mudo: o sistema o mostra conectado, mas o fone não responde.
    ///
    /// O app pede o nome do fone (pedido sem efeito) e espera uma notificação. Sem
    /// resposta, derruba o elo; o laço principal reconecta.
    async fn check_link(&self) {
        let Some(dev) = self.inner.borrow().device.clone() else { return };
        let now = Instant::now();
        let (sent, due) = {
            let i = self.inner.borrow();
            (i.probe_sent, i.probe_due)
        };
        if let Some(sent) = sent {
            if dev.last_rx().is_some_and(|rx| rx >= sent) {
                let mut i = self.inner.borrow_mut();
                i.probe_sent = None;
                i.probe_due = now + PROBE_IDLE;
                return;
            }
            if now.duration_since(sent) < PROBE_TIMEOUT {
                return;
            }
        } else if now < due {
            return;
        } else {
            self.inner.borrow_mut().probe_sent = Some(now);
            let frame = p::cmd_request(&[p::cmd::PAIR_NAME]).expect("pedido do nome");
            match dev.send_command(&frame).await {
                Ok(()) => return,
                // Qualquer falha de escrita indica elo morto.
                Err(e) => log::info!("sonda do controle falhou: {e}"),
            }
        }
        log::info!("o fone não respondeu; o app derruba o elo de controle e reconecta");
        self.inner.borrow_mut().probe_sent = None;
        self.status("O fone parou de responder. Reconectando…");
        if tokio::time::timeout(Duration::from_secs(6), dev.disconnect()).await.is_err() {
            log::debug!("falha ao derrubar o elo");
        }
    }

    /// Áudio conectado no sistema. Sem suporte ou sem endereço, assume que sim.
    async fn audio_up(&self) -> bool {
        match self.classic_mac() {
            Some(mac) if SUPPORTED => classic::status(&mac).await.connected,
            _ => true,
        }
    }

    fn set_audio_only(&self, on: bool) {
        let changed = {
            let mut i = self.inner.borrow_mut();
            let c = i.audio_only != on;
            i.audio_only = on;
            c
        };
        if changed {
            (self.emit)(Event::AudioOnly(on));
        }
    }

    /// O HT05 para de anunciar o BLE cerca de 60 s depois de conectar o áudio.
    ///
    /// Nesse estado o app reconhece o fone pelo sistema, mas não alcança o controle.
    /// O app não reconecta o áudio sozinho: o usuário decide (ver `wake`).
    async fn check_audio_only(&self) {
        if self.inner.borrow().waking || !SUPPORTED {
            return;
        }
        let Some(mac) = self.classic_mac() else { return };
        let st = classic::status(&mac).await;
        self.set_audio_only(st.connected);
        if st.connected {
            self.status("Áudio conectado. Controles em espera.");
        }
    }

    /// Garante o áudio clássico conectado (Linux). Repete a cada 15 s, ou até 60 s após falha.
    async fn check_audio(&self, force: bool) {
        if !SUPPORTED {
            return;
        }
        let Some(mac) = self.classic_mac() else { return };
        if !force && Instant::now() < self.inner.borrow().next_audio_check {
            return;
        }
        let st = classic::status(&mac).await;
        if st.connected {
            let mut i = self.inner.borrow_mut();
            i.audio_fails = 0;
            i.audio_seen = true;
            i.next_audio_check = Instant::now() + Duration::from_secs(15);
            return;
        }
        if self.inner.borrow().audio_seen && !force {
            // O áudio estava conectado e caiu com o controle ativo: o usuário o
            // desconectou no sistema. O app não reconecta contra a vontade dele.
            self.inner.borrow_mut().next_audio_check = Instant::now() + Duration::from_secs(5);
            self.status("Controle conectado; áudio desconectado");
            return;
        }
        self.status("Conectando o áudio…");
        let mut result = classic::ensure_connected(&mac).await;
        log::info!("áudio: {result}");
        let ok = result == "conectado";
        // Falha: tenta de novo em 3, 6, 12, 24, 48 e depois a cada 60 s.
        let fails = {
            let mut i = self.inner.borrow_mut();
            i.audio_fails = if ok { 0 } else { i.audio_fails + 1 };
            let wait = if ok { 15.0 } else { (1.5 * 2f64.powi(i.audio_fails as i32)).min(60.0) };
            i.next_audio_check = Instant::now() + Duration::from_secs_f64(wait);
            i.audio_fails
        };
        if !ok && fails >= 3 && result.contains("br-connection") {
            result = "o Bluetooth do sistema não responde. Desligue e ligue o Bluetooth.".into();
        }
        if ok {
            self.status("Conectado (controle e áudio)");
        } else {
            self.status(&format!("Controle conectado; áudio: {result}"));
        }
    }

    fn command(&self, cmd: Command) {
        let me = self.clone();
        tokio::task::spawn_local(async move {
            match cmd {
                Command::ConnectAudio => me.check_audio(true).await,
                Command::Wake => me.wake().await,
                Command::EnableBluetooth => me.enable_bluetooth().await,
                other => me.device_command(other).await,
            }
        });
    }

    /// Pedido do usuário: liga o Bluetooth do sistema (Linux).
    async fn enable_bluetooth(&self) {
        if !SUPPORTED {
            return;
        }
        if let Err(e) = classic::set_powered(true).await {
            // Bloqueio por rfkill, sem adaptador.
            log::info!("falha ao ligar o Bluetooth: {e}");
            self.error("O sistema não ligou o Bluetooth. Ligue-o nas configurações do sistema.");
        }
    }

    /// Pedido do usuário: reconecta o áudio (pausa curta) para o fone anunciar o controle.
    async fn wake(&self) {
        let Some(mac) = self.classic_mac() else { return };
        if self.inner.borrow().waking {
            return;
        }
        self.inner.borrow_mut().waking = true;
        self.status("Ativando os controles. O Bluetooth reinicia por alguns segundos…");
        let ok = classic::reconnect(&mac).await;
        self.set_audio_only(false);
        if !ok {
            self.error("Não foi possível reconectar o áudio. Desligue e ligue o Bluetooth do sistema.");
        }
        self.inner.borrow_mut().waking = false;
    }

    /// Executa um comando no fone sem bloquear a interface.
    async fn device_command(&self, cmd: Command) {
        let dev = self.inner.borrow().device.clone().filter(|d| d.is_connected());
        let Some(dev) = dev else {
            self.error("Fone desconectado.");
            return;
        };
        let res = match &cmd {
            Command::Noise(m) => dev.set_noise(m).await,
            Command::Eq(eq) => dev.set_eq(eq).await,
            Command::Key(g, a) => dev.set_key(*g, *a).await,
            Command::Game(on) => dev.set_game_mode(*on).await,
            Command::Sleep(on) => dev.set_sleep_mode(*on).await,
            Command::Rename(n) => dev.rename(n).await,
            _ => Ok(()),
        };
        match res {
            // Confere logo se o fone ainda responde, para o comando não se perder em silêncio.
            Ok(()) => {
                let mut i = self.inner.borrow_mut();
                i.probe_due = i.probe_due.min(Instant::now() + Duration::from_millis(1500));
            }
            Err(e) => {
                log::warn!("erro no comando {cmd:?}: {e}");
                self.error(&e.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(percent: Option<u8>, charging: bool) -> p::BatteryLevel {
        p::BatteryLevel { percent, charging }
    }

    #[test]
    fn fones_na_caixa() {
        let b = |l, r| p::Battery { left: l, right: r, case: lvl(Some(50), false) };
        assert!(in_case(Some(&b(lvl(Some(85), true), lvl(Some(89), true)))));
        assert!(in_case(Some(&b(lvl(None, false), lvl(Some(89), true)))));
        assert!(!in_case(Some(&b(lvl(Some(85), true), lvl(Some(89), false)))));
        assert!(!in_case(Some(&b(lvl(Some(85), false), lvl(Some(89), false)))));
        assert!(!in_case(None));
    }
}
