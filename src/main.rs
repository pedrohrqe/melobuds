//! Ponto de entrada da interface gráfica (qcyctl-gui).
//!
//! A interface Slint roda na thread principal. O controlador (BLE e BlueZ) roda numa
//! thread com um runtime tokio de uma só thread. Os eventos do controlador chegam à
//! interface por `invoke_from_event_loop`; os comandos vão por um canal.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use slint::winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tokio::sync::mpsc;

use melobuds::controller::{Command, Controller, Event, SharedSettings};
use melobuds::device::DeviceState;
use melobuds::protocol as p;
use melobuds::settings::Settings;
use melobuds::{autostart, tray};

slint::include_modules!();

const DEBOUNCE: Duration = Duration::from_millis(350);
const CREDIT_URL: &str = "https://github.com/pedrohrqe";

#[derive(Parser)]
#[command(name = "qcyctl-gui", about = "MeloBuds by @pedrohrqe")]
struct Args {
    /// endereço BLE do fone (padrão: procura)
    #[arg(short, long)]
    address: Option<String>,
    /// inicia só na bandeja
    #[arg(long)]
    hidden: bool,
    /// mostra mensagens de depuração
    #[arg(long)]
    debug: bool,
}

/// Lê a configuração de animações do GNOME. Em outros ambientes, retorna true.
fn desktop_animations_enabled() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() != "false")
        .unwrap_or(true)
}

fn open_url(url: &str) {
    #[cfg(windows)]
    let r = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        log::info!("falha ao abrir {url}: {e}");
    }
}

fn quick_modes() -> [p::NoiseMode; 6] {
    [
        p::NoiseMode::normal(),
        p::NoiseMode::anc("noisy", 3),
        p::NoiseMode::anc("commute", 3),
        p::NoiseMode::anc("indoor", 3),
        p::NoiseMode::transparency(4),
        p::NoiseMode::transparency(0),
    ]
}

/// Estado da interface: o último estado do fone mais as mudanças otimistas.
struct App {
    ui: MainWindow,
    s: RefCell<DeviceState>,
    settings: SharedSettings,
    cmd: mpsc::UnboundedSender<Command>,
    tray_tx: mpsc::UnboundedSender<tray::Model>,
    ignore_case: RefCell<bool>,
    pending_noise: RefCell<Option<p::NoiseMode>>,
    pending_eq: RefCell<Option<p::Equalizer>>,
    noise_timer: Timer,
    eq_timer: Timer,
    visible: RefCell<bool>,
    focused: RefCell<bool>,
    tray_ok: bool,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

fn with_app(f: impl FnOnce(&Rc<App>)) {
    APP.with(|a| {
        if let Some(app) = a.borrow().as_ref() {
            f(app)
        }
    });
}

impl App {
    fn backend(&self) -> Backend<'_> {
        self.ui.global::<Backend>()
    }

    fn send(&self, c: Command) {
        let _ = self.cmd.send(c);
    }

    fn toast(&self, text: &str, kind: &str) {
        self.backend().invoke_toast(text.into(), kind.into());
    }

    /// Copia o estado do fone para as propriedades da interface e para a bandeja.
    fn publish(&self) {
        let s = self.s.borrow();
        let b = self.backend();
        let ignore = *self.ignore_case.borrow();
        b.set_connected(s.connected);
        let name = s.pair_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| s.name.clone());
        let name = if name.trim().is_empty() { "QCY HT05".to_string() } else { name.trim().to_string() };
        b.set_name(name.as_str().into());
        b.set_firmware(
            match &s.version {
                Some((l, r)) if l == r => l.clone(),
                Some((l, r)) => format!("L {l} · R {r}"),
                None => String::new(),
            }
            .into(),
        );
        let addr = if s.connected { self.settings.lock().unwrap().get_str("ble_mac").unwrap_or_default() } else { String::new() };
        b.set_address(addr.into());
        let lvl = |x: Option<p::BatteryLevel>| match x {
            Some(x) => Battery { percent: x.percent.map_or(-1, i32::from), charging: x.charging },
            None => Battery { percent: -1, charging: false },
        };
        let bat = s.battery;
        b.set_battery(ModelRc::new(VecModel::from(vec![
            lvl(bat.map(|x| x.left)),
            lvl(bat.map(|x| x.right)),
            lvl(bat.filter(|_| !ignore).map(|x| x.case)), // "Ignorar caixa": descarta o dado
        ])));
        let n = s.noise;
        b.set_noise_kind(n.map_or(-1, |n| n.kind as i32));
        b.set_anc_scene(n.filter(|n| n.kind == p::NoiseKind::Anc).and_then(|n| n.scene).unwrap_or("noisy").into());
        b.set_anc_level(n.filter(|n| n.kind == p::NoiseKind::Anc).map_or(3, |n| n.level as i32));
        b.set_trans_level(n.filter(|n| n.kind == p::NoiseKind::Transparency).map_or(4, |n| n.level as i32));
        b.set_noise_label(n.map(|n| n.label()).unwrap_or_default().into());
        b.set_game_mode(s.game_mode.unwrap_or(false));
        b.set_sleep_mode(s.sleep_mode.unwrap_or(false));
        b.set_eq_preset(s.eq.as_ref().map_or(-1, |e| e.preset as i32));
        b.set_eq_gains(ModelRc::new(VecModel::from(self.eq_gains(&s))));
        b.set_keys(ModelRc::new(VecModel::from(
            p::Gesture::ALL.iter().map(|g| s.keys.get(&(*g as u8)).map_or(-1, |a| *a as i32)).collect::<Vec<_>>(),
        )));

        // Bandeja.
        let battery = match bat {
            Some(x) if ignore => format!("L {} · R {}", x.left.label(), x.right.label()),
            Some(x) => format!("L {} · R {} · caixa {}", x.left.label(), x.right.label(), x.case.label()),
            None => String::new(),
        };
        let model = tray::Model {
            connected: s.connected,
            battery,
            name,
            mode_label: n.map(|n| n.label()).unwrap_or_else(|| "?".into()),
            quick_mode: n.and_then(|n| quick_modes().iter().position(|m| *m == n)),
            game_mode: s.game_mode.unwrap_or(false),
        };
        #[cfg(windows)]
        tray::Handle.update(model.clone());
        let _ = self.tray_tx.send(model);
    }

    fn eq_gains(&self, s: &DeviceState) -> Vec<i32> {
        s.eq.as_ref().map_or_else(|| vec![0; p::EQ_BANDS.len()], |e| e.gains.clone())
    }

    fn on_event(&self, ev: Event) {
        match ev {
            Event::State(s) => {
                *self.s.borrow_mut() = s;
                self.publish();
            }
            Event::Status(t) => self.backend().set_status(t.into()),
            Event::Error(t) => self.toast(&t, "error"),
            Event::Bluetooth(on) => self.backend().set_bluetooth_on(on),
            Event::AudioOnly(on) => self.backend().set_audio_only(on),
        }
    }

    // ---- comandos da interface ----

    fn set_noise(self: &Rc<Self>, kind: i32, scene: &str, level: i32) {
        let mode = match kind {
            1 => {
                let scene = p::ANC_SCENES.iter().map(|s| s.0).find(|k| *k == scene).unwrap_or("noisy");
                let level = if scene == "wind" { 1 } else { level.clamp(1, p::ANC_LEVELS as i32) };
                p::NoiseMode::anc(scene, level as u8)
            }
            2 => p::NoiseMode::transparency(level.clamp(0, 6) as u8),
            _ => p::NoiseMode::normal(),
        };
        self.s.borrow_mut().noise = Some(mode);
        self.publish();
        *self.pending_noise.borrow_mut() = Some(mode);
        let me = Rc::downgrade(self);
        self.noise_timer.start(TimerMode::SingleShot, DEBOUNCE, move || {
            if let Some(me) = me.upgrade() {
                if let Some(m) = me.pending_noise.borrow_mut().take() {
                    me.send(Command::Noise(m));
                }
            }
        });
    }

    fn set_eq(self: &Rc<Self>, eq: p::Equalizer) {
        self.s.borrow_mut().eq = Some(eq.clone());
        self.publish();
        *self.pending_eq.borrow_mut() = Some(eq);
        let me = Rc::downgrade(self);
        self.eq_timer.start(TimerMode::SingleShot, DEBOUNCE, move || {
            if let Some(me) = me.upgrade() {
                if let Some(eq) = me.pending_eq.borrow_mut().take() {
                    me.send(Command::Eq(eq));
                }
            }
        });
    }

    fn set_eq_preset(self: &Rc<Self>, index: i32) {
        let Some((_, gains)) = p::EQ_PRESETS.get(index as usize) else { return };
        self.set_eq(p::Equalizer { preset: index as u8, gains: gains.to_vec() });
    }

    fn set_eq_gain(self: &Rc<Self>, band: i32, db: i32) {
        let mut gains = self.eq_gains(&self.s.borrow());
        let Some(g) = gains.get_mut(band as usize) else { return };
        *g = db.clamp(p::EQ_MIN_DB, p::EQ_MAX_DB);
        self.set_eq(p::Equalizer::custom(&gains));
    }

    fn set_key(&self, gesture: i32, action: i32) {
        self.s.borrow_mut().keys.insert(gesture as u8, action as u8);
        self.publish();
        self.send(Command::Key(gesture as u8, action as u8));
    }

    fn set_game(&self, on: bool) {
        self.s.borrow_mut().game_mode = Some(on);
        self.publish();
        self.send(Command::Game(on));
    }

    fn set_sleep(&self, on: bool) {
        self.s.borrow_mut().sleep_mode = Some(on);
        self.publish();
        self.send(Command::Sleep(on));
    }

    fn rename(&self, name: &str) -> bool {
        let name = name.trim();
        if let Err(e) = p::cmd_rename(name) {
            self.toast(&e.to_string(), "error");
            return false;
        }
        self.s.borrow_mut().pair_name = Some(name.to_string());
        self.publish();
        self.send(Command::Rename(name.to_string()));
        self.toast("Nome enviado. Ele aparece nos outros aparelhos depois de reconectar.", "info");
        true
    }

    fn set_autostart(&self, on: bool) {
        if on != autostart::is_enabled() {
            if let Err(e) = autostart::set_enabled(on) {
                self.toast(&format!("O sistema não salvou a inicialização automática: {e}"), "error");
            }
        }
        self.backend().set_autostart(autostart::is_enabled());
    }

    // ---- janela ----

    fn update_live(&self) {
        self.ui.set_live(*self.visible.borrow() && *self.focused.borrow());
    }

    fn show(&self) {
        *self.visible.borrow_mut() = true;
        *self.focused.borrow_mut() = true;
        let _ = self.ui.show();
        self.ui.window().with_winit_window(|w| {
            w.set_minimized(false);
            w.focus_window();
        });
        self.update_live();
    }

    fn hide(&self) {
        *self.visible.borrow_mut() = false;
        let _ = self.ui.hide();
        self.update_live();
    }

    fn tray_action(self: &Rc<Self>, a: tray::Action) {
        match a {
            tray::Action::ToggleWindow => {
                if *self.visible.borrow() {
                    self.hide()
                } else {
                    self.show()
                }
            }
            tray::Action::ShowWindow => self.show(),
            tray::Action::QuickMode(i) => {
                if let Some(m) = quick_modes().get(i) {
                    self.send(Command::Noise(*m));
                }
            }
            tray::Action::Game(on) => self.send(Command::Game(on)),
            tray::Action::ConnectAudio => self.send(Command::ConnectAudio),
            tray::Action::Quit => {
                let _ = slint::quit_event_loop();
            }
        }
    }
}

fn wire(app: &Rc<App>) {
    let b = app.backend();
    macro_rules! cb {
        ($reg:ident, |$me:ident $(, $a:ident)*| $body:expr) => {{
            let weak = Rc::downgrade(app);
            b.$reg(move |$($a),*| {
                let $me = weak.upgrade().expect("app");
                $body
            });
        }};
    }
    cb!(on_set_noise, |me, k, s, l| me.set_noise(k, s.as_str(), l));
    cb!(on_set_eq_preset, |me, i| me.set_eq_preset(i));
    cb!(on_set_eq_gain, |me, band, db| me.set_eq_gain(band, db));
    cb!(on_set_key, |me, g, a| me.set_key(g, a));
    cb!(on_set_game, |me, on| me.set_game(on));
    cb!(on_set_sleep, |me, on| me.set_sleep(on));
    cb!(on_rename, |me, name| me.rename(name.as_str()));
    cb!(on_enable_bluetooth, |me| me.send(Command::EnableBluetooth));
    cb!(on_wake_control, |me| me.send(Command::Wake));
    cb!(on_connect_audio, |me| me.send(Command::ConnectAudio));
    cb!(on_set_reduce_motion, |me, on| {
        me.backend().set_reduce_motion(on);
        let _ = me.settings.lock().unwrap().set_bool("reduce_motion", on);
    });
    cb!(on_set_ignore_case, |me, on| {
        *me.ignore_case.borrow_mut() = on;
        me.backend().set_ignore_case(on);
        let _ = me.settings.lock().unwrap().set_bool("ignore_case", on);
        me.publish(); // a lista de bateria muda
    });
    cb!(on_set_autostart, |me, on| me.set_autostart(on));
    b.on_open_url(|url| open_url(url.as_str()));
}

fn main() {
    let args = Args::parse();
    let mut log = env_logger::Builder::new();
    log.filter_level(if args.debug { log::LevelFilter::Debug } else { log::LevelFilter::Warn });
    if args.debug {
        // As bibliotecas de interface e de D-Bus são verbosas demais em debug.
        for m in ["zbus", "winit", "i_slint", "sctk", "calloop", "tracing", "fontdb", "cosmic_text", "naga", "wgpu"] {
            log.filter_module(m, log::LevelFilter::Warn);
        }
    }
    log.init();

    let settings: SharedSettings = Arc::new(Mutex::new(Settings::load()));
    let ui = MainWindow::new().expect("falha ao carregar a interface");

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (tray_tx, tray_rx) = mpsc::unbounded_channel::<tray::Model>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<bool>();

    let emit: melobuds::controller::Emit = Arc::new(|ev| {
        let _ = slint::invoke_from_event_loop(move || with_app(|a| a.on_event(ev)));
    });
    let on_tray: tray::OnAction = Arc::new(|a| {
        let _ = slint::invoke_from_event_loop(move || with_app(|app| app.tray_action(a)));
    });

    // Núcleo: BLE, BlueZ e bandeja (Linux) numa thread própria.
    let core_settings = settings.clone();
    let address = args.address.clone();
    #[cfg(target_os = "linux")]
    let core_tray = on_tray.clone();
    std::thread::Builder::new()
        .name("nucleo".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                #[cfg(target_os = "linux")]
                {
                    let handle = tray::spawn(core_tray).await;
                    let _ = ready_tx.send(handle.is_some());
                    if let Some(h) = handle {
                        let mut rx = tray_rx;
                        tokio::task::spawn_local(async move {
                            while let Some(m) = rx.recv().await {
                                h.update(m).await;
                            }
                        });
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let _ = (ready_tx, tray_rx);
                }
                // kill/Ctrl+C: fecha o app pela interface.
                tokio::task::spawn_local(async {
                    #[cfg(unix)]
                    {
                        use tokio::signal::unix::{signal, SignalKind};
                        let (mut t, mut i) = (signal(SignalKind::terminate()).unwrap(), signal(SignalKind::interrupt()).unwrap());
                        tokio::select! { _ = t.recv() => {}, _ = i.recv() => {} }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = tokio::signal::ctrl_c().await;
                    }
                    let _ = slint::invoke_from_event_loop(|| {
                        let _ = slint::quit_event_loop();
                    });
                });
                Controller::new(address, core_settings, emit).run(cmd_rx).await;
            });
        })
        .expect("thread do núcleo");

    // Bandeja: no Linux o núcleo informa se ela existe; no Windows ela nasce aqui.
    #[cfg(target_os = "linux")]
    let tray_ok = ready_rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false);
    #[cfg(windows)]
    let (tray_ok, win_tray) = {
        let _ = ready_rx;
        let h = tray::spawn(on_tray.clone());
        (h.is_some(), h)
    };
    #[cfg(not(any(target_os = "linux", windows)))]
    let tray_ok = {
        let _ = (ready_rx, on_tray);
        false
    };

    let (reduce, ignore_case) = {
        let s = settings.lock().unwrap();
        let reduce = match s.get_str("reduce_motion") {
            Some(_) => s.get_bool("reduce_motion", false),
            None => !desktop_animations_enabled(),
        };
        (reduce, s.get_bool("ignore_case", false))
    };

    let app = Rc::new(App {
        ui,
        s: RefCell::new(DeviceState::default()),
        settings,
        cmd: cmd_tx,
        tray_tx,
        ignore_case: RefCell::new(ignore_case),
        pending_noise: RefCell::new(None),
        pending_eq: RefCell::new(None),
        noise_timer: Timer::default(),
        eq_timer: Timer::default(),
        visible: RefCell::new(false),
        focused: RefCell::new(false),
        tray_ok,
    });

    let b = app.backend();
    b.set_app_name(tray::APP_NAME.into());
    b.set_credit_url(CREDIT_URL.into());
    b.set_credit_text(CREDIT_URL.trim_start_matches("https://").into());
    b.set_tray_available(tray_ok);
    b.set_can_enable_bluetooth(cfg!(target_os = "linux"));
    b.set_reduce_motion(reduce);
    b.set_ignore_case(ignore_case);
    b.set_autostart(autostart::is_enabled());
    b.set_eq_bands(ModelRc::new(VecModel::from(p::EQ_BANDS.iter().map(|s| SharedString::from(*s)).collect::<Vec<_>>())));
    b.set_eq_presets(ModelRc::new(VecModel::from(p::EQ_PRESETS.iter().map(|s| SharedString::from(s.0)).collect::<Vec<_>>())));
    b.set_anc_scenes(ModelRc::new(VecModel::from(
        p::ANC_SCENES.iter().map(|s| AncScene { key: s.0.into(), label: s.1.into() }).collect::<Vec<_>>(),
    )));
    b.set_key_actions(ModelRc::new(VecModel::from(
        p::KEY_ACTIONS.iter().map(|a| KeyAction { value: a.0 as i32, label: a.1.into() }).collect::<Vec<_>>(),
    )));
    b.set_eq_min(p::EQ_MIN_DB);
    b.set_eq_max(p::EQ_MAX_DB);
    wire(&app);
    app.publish();

    // Fechar a janela: vai para a bandeja, se ela existe; senão encerra o app.
    let weak = Rc::downgrade(&app);
    app.ui.window().on_close_requested(move || {
        if let Some(app) = weak.upgrade() {
            if app.tray_ok {
                app.hide();
            } else {
                let _ = slint::quit_event_loop();
            }
        }
        slint::CloseRequestResponse::HideWindow
    });
    // Foco e minimização: os laços de animação só rodam com a janela em uso.
    let weak = Rc::downgrade(&app);
    app.ui.window().on_winit_window_event(move |_w, ev| {
        if let Some(app) = weak.upgrade() {
            match ev {
                WindowEvent::Focused(f) => {
                    *app.focused.borrow_mut() = *f;
                    app.update_live();
                }
                WindowEvent::Occluded(o) if *o => {
                    *app.focused.borrow_mut() = false;
                    app.update_live();
                }
                _ => {}
            }
        }
        EventResult::Propagate
    });

    APP.with(|a| *a.borrow_mut() = Some(app.clone()));
    #[cfg(windows)]
    let _win_tray = win_tray;

    if !(args.hidden && tray_ok) {
        app.show();
    }
    slint::run_event_loop_until_quit().expect("laço de eventos");
    // O elo BLE fica aberto de propósito (ver controller.rs): a próxima abertura do app
    // encontra o fone sem reconectar o áudio.
    std::process::exit(0);
}
