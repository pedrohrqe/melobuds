//! Conexão BLE com o QCY HT05 e estado do dispositivo (btleplug).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use btleplug::api::{Central, CentralEvent, CentralState, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::StreamExt;
use uuid::Uuid;

use crate::protocol as p;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceState {
    pub connected: bool,
    pub name: String,
    pub battery: Option<p::Battery>,
    pub version: Option<(String, String)>,
    pub noise: Option<p::NoiseMode>,
    pub game_mode: Option<bool>,
    pub sleep_mode: Option<bool>,
    pub balance: Option<u8>,
    pub pair_name: Option<String>,
    pub eq: Option<p::Equalizer>,
    pub keys: BTreeMap<u8, u8>,
}

/// O Bluetooth do sistema está desligado ou não existe (equivale ao BleakBluetoothNotAvailableError).
#[derive(Debug)]
pub struct BluetoothOff;

impl std::fmt::Display for BluetoothOff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bluetooth desligado")
    }
}
impl std::error::Error for BluetoothOff {}

/// Primeiro adaptador Bluetooth do sistema. Falha com `BluetoothOff` se não há nenhum.
pub async fn adapter() -> Result<Adapter> {
    let manager = Manager::new().await.context("Bluetooth indisponível")?;
    manager.adapters().await?.into_iter().next().ok_or_else(|| BluetoothOff.into())
}

fn parse_qcy(data: &HashMap<u16, Vec<u8>>) -> Option<p::Advertisement> {
    [p::COMPANY_ID, p::COMPANY_ID_ALT]
        .iter()
        .find_map(|cid| data.get(cid).and_then(|payload| p::parse_advertisement(payload)))
}

pub struct Found {
    pub peripheral: Peripheral,
    pub address: String,
    pub name: String,
    pub advertisement: p::Advertisement,
}

/// Retorna o primeiro fone QCY que anunciar, sem esperar o fim do scan.
///
/// Só conta um anúncio recebido durante este scan. O BlueZ guarda dispositivos antigos
/// com os dados do último anúncio; um fone guardado que não anuncia não serve.
pub async fn find_first_with_adv(adapter: &Adapter, timeout: Duration) -> Result<Option<Found>> {
    if adapter.adapter_state().await.ok() == Some(CentralState::PoweredOff) {
        return Err(BluetoothOff.into());
    }
    let mut events = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await.map_err(|e| {
        log::info!("scan falhou: {e}");
        anyhow::Error::from(BluetoothOff)
    })?;
    let deadline = tokio::time::Instant::now() + timeout;
    let result = loop {
        let ev = match tokio::time::timeout_at(deadline, events.next()).await {
            Ok(Some(ev)) => ev,
            _ => break None,
        };
        let id: PeripheralId = match ev {
            CentralEvent::ManufacturerDataAdvertisement { id, .. } => id,
            CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
            CentralEvent::StateUpdate(CentralState::PoweredOff) => {
                let _ = adapter.stop_scan().await;
                return Err(BluetoothOff.into());
            }
            _ => continue,
        };
        let Ok(per) = adapter.peripheral(&id).await else { continue };
        let Ok(Some(props)) = per.properties().await else { continue };
        if let Some(adv) = parse_qcy(&props.manufacturer_data) {
            break Some(Found {
                address: props.address.to_string(),
                name: props.local_name.unwrap_or_else(|| "QCY".into()),
                peripheral: per,
                advertisement: adv,
            });
        }
    };
    let _ = adapter.stop_scan().await;
    Ok(result)
}

/// Lista os fones QCY que anunciam durante `timeout` (para a linha de comando).
pub async fn discover(adapter: &Adapter, timeout: Duration, all: bool) -> Result<Vec<Found>> {
    adapter.start_scan(ScanFilter::default()).await?;
    tokio::time::sleep(timeout).await;
    let mut out = Vec::new();
    for per in adapter.peripherals().await? {
        let Ok(Some(props)) = per.properties().await else { continue };
        if props.rssi.is_none() {
            continue; // guardado pelo BlueZ, mas não anunciou agora
        }
        let adv = parse_qcy(&props.manufacturer_data);
        if adv.is_some() || all {
            out.push(Found {
                address: props.address.to_string(),
                name: props.local_name.clone().unwrap_or_else(|| "?".into()),
                peripheral: per,
                advertisement: adv.unwrap_or({
                    let none = p::BatteryLevel { percent: None, charging: false };
                    p::Advertisement { vendor_id: 0, battery: p::Battery { left: none, right: none, case: none }, classic_mac: String::new() }
                }),
            });
        }
    }
    let _ = adapter.stop_scan().await;
    Ok(out)
}

pub type Listener = Arc<dyn Fn(DeviceState) + Send + Sync>;

/// Cliente do HT05. As escritas passam por uma fila serial, como no app oficial.
pub struct HT05 {
    per: Peripheral,
    chars: HashMap<Uuid, Characteristic>,
    state: Mutex<DeviceState>,
    lock: tokio::sync::Mutex<()>,
    connected: AtomicBool,
    last_rx: Mutex<Option<Instant>>,
    listener: Mutex<Option<Listener>>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

fn uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).expect("UUID do protocolo")
}

impl HT05 {
    /// Conecta, assina as notificações e lê o estado inicial.
    pub async fn connect(adapter: &Adapter, per: Peripheral, timeout: Duration) -> Result<Arc<Self>> {
        match Self::try_connect(adapter, per.clone(), timeout).await {
            Ok(dev) => Ok(dev),
            Err(e) => {
                // Sem esta limpeza, o BlueZ mantém o fone como "conectado" e toda tentativa
                // seguinte falha com "Not connected".
                if tokio::time::timeout(Duration::from_secs(5), per.disconnect()).await.is_err() {
                    log::debug!("falha na limpeza da conexão");
                }
                Err(e)
            }
        }
    }

    async fn try_connect(adapter: &Adapter, per: Peripheral, timeout: Duration) -> Result<Arc<Self>> {
        tokio::time::timeout(timeout, per.connect()).await.map_err(|_| anyhow!("tempo esgotado ao conectar"))??;
        per.discover_services().await?;
        let service = uuid(p::SERVICE);
        let svc = per
            .services()
            .into_iter()
            .find(|s| s.uuid == service)
            .ok_or_else(|| anyhow!("serviço de controle QCY (A001) não encontrado"))?;
        // O HT05 repete UUIDs em outro serviço, por isso a busca é dentro do A001.
        let chars: HashMap<Uuid, Characteristic> = svc.characteristics.into_iter().map(|c| (c.uuid, c)).collect();
        let name = per.properties().await.ok().flatten().and_then(|p| p.local_name).unwrap_or_else(|| "QCY HT05".into());
        let dev = Arc::new(Self {
            per: per.clone(),
            chars,
            state: Mutex::new(DeviceState { connected: true, name, ..Default::default() }),
            lock: tokio::sync::Mutex::new(()),
            connected: AtomicBool::new(true),
            last_rx: Mutex::new(None),
            listener: Mutex::new(None),
            tasks: Mutex::new(Vec::new()),
        });

        // Notificações do fone.
        let mut notes = per.notifications().await?;
        let weak = Arc::downgrade(&dev);
        let t1 = tokio::spawn(async move {
            while let Some(n) = notes.next().await {
                let Some(dev) = weak.upgrade() else { break };
                dev.on_notify(n.uuid, &n.value);
            }
        });
        // Queda do elo: o sistema avisa pelo evento do adaptador.
        let mut events = adapter.events().await?;
        let id = per.id();
        let weak = Arc::downgrade(&dev);
        let t2 = tokio::spawn(async move {
            while let Some(ev) = events.next().await {
                if let CentralEvent::DeviceDisconnected(x) = ev {
                    if x == id {
                        if let Some(dev) = weak.upgrade() {
                            dev.on_disconnect();
                        }
                        break;
                    }
                }
            }
        });
        dev.tasks.lock().unwrap().extend([t1, t2]);

        for u in [p::CHAR_RESP, p::CHAR_BATTERY] {
            if let Some(c) = dev.chars.get(&uuid(u)) {
                per.subscribe(c).await?;
            }
        }
        dev.refresh().await?;
        Ok(dev)
    }

    pub fn on_change(&self, listener: Listener) {
        *self.listener.lock().unwrap() = Some(listener);
    }

    fn emit(&self) {
        let st = self.state();
        if let Some(cb) = self.listener.lock().unwrap().clone() {
            cb(st);
        }
    }

    pub fn state(&self) -> DeviceState {
        self.state.lock().unwrap().clone()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Instante da última notificação do fone.
    pub fn last_rx(&self) -> Option<Instant> {
        *self.last_rx.lock().unwrap()
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.per.disconnect().await?;
        self.on_disconnect();
        Ok(())
    }

    fn on_disconnect(&self) {
        if self.connected.swap(false, Ordering::Relaxed) {
            self.state.lock().unwrap().connected = false;
            self.emit();
        }
    }

    // --- I/O de baixo nível ---

    fn char(&self, u: &str) -> Result<&Characteristic> {
        self.chars.get(&uuid(u)).ok_or_else(|| anyhow!("característica {} ausente", &u[4..8]))
    }

    async fn write(&self, u: &str, data: &[u8]) -> Result<()> {
        if !self.is_connected() {
            bail!("fone desconectado");
        }
        let c = self.char(u)?;
        let _g = self.lock.lock().await;
        log::debug!("-> {} {}", &u[4..8], hex(data));
        self.per.write(c, data, WriteType::WithoutResponse).await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }

    async fn read(&self, u: &str) -> Result<Vec<u8>> {
        if !self.is_connected() {
            bail!("fone desconectado");
        }
        let c = self.char(u)?;
        let _g = self.lock.lock().await;
        Ok(self.per.read(c).await?)
    }

    pub async fn send_command(&self, frame: &[u8]) -> Result<()> {
        self.write(p::CHAR_CMD, frame).await
    }

    fn on_notify(&self, u: Uuid, data: &[u8]) {
        *self.last_rx.lock().unwrap() = Some(Instant::now());
        let s = u.to_string();
        log::debug!("<- {} {}", &s[4..8], hex(data));
        let ok = {
            let mut st = self.state.lock().unwrap();
            if s == p::CHAR_BATTERY {
                p::Battery::decode(data).map(|b| st.battery = Some(b)).map_err(|e| e.to_string())
            } else if s == p::CHAR_RESP {
                p::parse_frame(data).map(|t| apply_tlvs(&mut st, &t)).map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        };
        match ok {
            Ok(()) => self.emit(),
            Err(e) => log::warn!("{e}"),
        }
    }

    // --- leitura de estado ---

    /// Lê todo o estado do fone.
    pub async fn refresh(&self) -> Result<()> {
        let resp = p::parse_frame(&self.read(p::CHAR_RESP).await?)?;
        let battery = p::Battery::decode(&self.read(p::CHAR_BATTERY).await?)?;
        let version = p::decode_version(&self.read(p::CHAR_VERSION).await?);
        let eq = p::Equalizer::decode(&self.read(p::CHAR_EQ).await?)?;
        let keys = p::decode_keys(&self.read(p::CHAR_KEYS).await?);
        {
            let mut st = self.state.lock().unwrap();
            apply_tlvs(&mut st, &resp);
            st.battery = Some(battery);
            st.version = Some(version);
            st.eq = Some(eq);
            st.keys = keys;
        }
        // A resposta do nome chega por notificação.
        self.send_command(&p::cmd_request(&[p::cmd::PAIR_NAME])?).await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        self.emit();
        Ok(())
    }

    // --- comandos ---

    pub async fn rename(&self, name: &str) -> Result<()> {
        self.send_command(&p::cmd_rename(name)?).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.send_command(&p::cmd_request(&[p::cmd::PAIR_NAME])?).await
    }

    pub async fn set_noise(&self, mode: &p::NoiseMode) -> Result<()> {
        self.send_command(&p::cmd_noise(mode)?).await
    }

    pub async fn set_game_mode(&self, on: bool) -> Result<()> {
        self.send_command(&p::cmd_set(p::cmd::GAME_MODE, if on { p::ON } else { p::OFF })).await
    }

    pub async fn set_sleep_mode(&self, on: bool) -> Result<()> {
        self.send_command(&p::cmd_set(p::cmd::SLEEP_MODE, if on { p::ON } else { p::OFF })).await
    }

    pub async fn set_eq(&self, eq: &p::Equalizer) -> Result<()> {
        self.write(p::CHAR_EQ, &eq.encode()?).await?;
        let eq = p::Equalizer::decode(&self.read(p::CHAR_EQ).await?)?;
        self.state.lock().unwrap().eq = Some(eq);
        self.emit();
        Ok(())
    }

    pub async fn set_key(&self, gesture: u8, action: u8) -> Result<()> {
        self.write(p::CHAR_KEYS, &p::encode_key(gesture, action)?).await?;
        let keys = p::decode_keys(&self.read(p::CHAR_KEYS).await?);
        self.state.lock().unwrap().keys = keys;
        self.emit();
        Ok(())
    }

    /// Escreve bytes crus numa característica (linha de comando `raw`).
    pub async fn write_raw(&self, u: &str, data: &[u8]) -> Result<()> {
        self.write(u, data).await
    }
}

impl Drop for HT05 {
    fn drop(&mut self) {
        for t in self.tasks.get_mut().unwrap().drain(..) {
            t.abort();
        }
    }
}

fn apply_tlvs(st: &mut DeviceState, tlvs: &[(u8, Vec<u8>)]) {
    for (cmd_id, payload) in tlvs {
        let Some(&value) = payload.first() else { continue };
        match *cmd_id {
            p::cmd::PAIR_NAME => st.pair_name = Some(p::decode_name(payload)),
            p::cmd::NOISE_MODE => match p::NoiseMode::decode(value) {
                Ok(m) => st.noise = Some(m),
                Err(e) => log::warn!("{e}"),
            },
            p::cmd::GAME_MODE => st.game_mode = Some(value == p::ON),
            p::cmd::SLEEP_MODE => st.sleep_mode = Some(value == p::ON),
            p::cmd::BALANCE => st.balance = Some(value),
            _ => {}
        }
    }
}

pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}
