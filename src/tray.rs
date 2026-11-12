//! Ícone na bandeja: bateria no tooltip, troca rápida de modo, modo jogo e áudio.
//!
//! Linux: StatusNotifierItem (ksni), no runtime tokio do núcleo.
//! Windows: tray-icon, na thread da interface (o laço de mensagens do winit o atende).

use std::sync::Arc;

pub const APP_NAME: &str = "MeloBuds by @pedrohrqe";

/// Atalhos de modo de ruído do menu. O índice vai em `Action::QuickMode`.
pub const QUICK_MODES: [&str; 6] = [
    "Normal",
    "ANC (ruidoso)",
    "ANC (transporte)",
    "ANC (interno)",
    "Transparência",
    "Transparência (voz)",
];

/// O que a bandeja mostra.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub connected: bool,
    /// Ex.: "L 80% · R 75% · caixa 40%"; vazio quando desconectado.
    pub battery: String,
    pub name: String,
    pub mode_label: String,
    /// Atalho que corresponde ao modo atual, se algum corresponde.
    pub quick_mode: Option<usize>,
    pub game_mode: bool,
}

impl Model {
    fn battery_line(&self) -> String {
        if self.connected {
            if self.battery.is_empty() { "Bateria: ?".into() } else { self.battery.clone() }
        } else {
            "Desconectado".into()
        }
    }

    fn tooltip(&self) -> (String, String) {
        if !self.connected {
            return (format!("{APP_NAME} — desconectado"), String::new());
        }
        let name = if self.name.is_empty() { "QCY HT05" } else { &self.name };
        (name.to_string(), format!("{}\n{}", self.battery_line(), self.mode_label))
    }
}

/// Ações pedidas pelo usuário na bandeja.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Clique no ícone: mostra a janela ou esconde, se já visível.
    ToggleWindow,
    ShowWindow,
    QuickMode(usize),
    Game(bool),
    ConnectAudio,
    Quit,
}

pub type OnAction = Arc<dyn Fn(Action) + Send + Sync>;

const ON_32: &[u8] = include_bytes!("../assets/tray-on-32.rgba");
const ON_64: &[u8] = include_bytes!("../assets/tray-on-64.rgba");
const OFF_32: &[u8] = include_bytes!("../assets/tray-off-32.rgba");
const OFF_64: &[u8] = include_bytes!("../assets/tray-off-64.rgba");

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem};
    use ksni::{MenuItem, ToolTip, TrayMethods};

    struct Tray {
        model: Model,
        on_action: OnAction,
    }

    /// RGBA → ARGB32 big-endian, o formato do StatusNotifierItem.
    fn argb(rgba: &[u8], size: i32) -> ksni::Icon {
        let mut data = Vec::with_capacity(rgba.len());
        for px in rgba.chunks_exact(4) {
            data.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
        }
        ksni::Icon { width: size, height: size, data }
    }

    impl ksni::Tray for Tray {
        fn id(&self) -> String {
            "melobuds".into()
        }
        fn title(&self) -> String {
            APP_NAME.into()
        }
        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            if self.model.connected {
                vec![argb(ON_32, 32), argb(ON_64, 64)]
            } else {
                vec![argb(OFF_32, 32), argb(OFF_64, 64)]
            }
        }
        fn tool_tip(&self) -> ToolTip {
            let (title, description) = self.model.tooltip();
            ToolTip { title, description, ..Default::default() }
        }
        fn activate(&mut self, _x: i32, _y: i32) {
            (self.on_action)(Action::ToggleWindow);
        }
        fn menu(&self) -> Vec<MenuItem<Self>> {
            let on = self.model.connected;
            let mut items: Vec<MenuItem<Self>> = vec![
                StandardItem { label: self.model.battery_line(), enabled: false, ..Default::default() }.into(),
                MenuItem::Separator,
                RadioGroup {
                    // Sem atalho correspondente, nenhuma opção fica marcada.
                    selected: self.model.quick_mode.unwrap_or(usize::MAX),
                    select: Box::new(|t: &mut Self, i| (t.on_action)(Action::QuickMode(i))),
                    options: QUICK_MODES
                        .iter()
                        .map(|l| RadioItem { label: (*l).into(), enabled: on, ..Default::default() })
                        .collect(),
                }
                .into(),
                MenuItem::Separator,
                CheckmarkItem {
                    label: "Modo jogo".into(),
                    enabled: on,
                    checked: self.model.game_mode,
                    activate: Box::new(|t: &mut Self| {
                        let next = !t.model.game_mode;
                        (t.on_action)(Action::Game(next))
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
            ];
            items.push(
                StandardItem {
                    label: "Conectar áudio".into(),
                    enabled: on,
                    activate: Box::new(|t: &mut Self| (t.on_action)(Action::ConnectAudio)),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Abrir janela".into(),
                    activate: Box::new(|t: &mut Self| (t.on_action)(Action::ShowWindow)),
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Sair".into(),
                    activate: Box::new(|t: &mut Self| (t.on_action)(Action::Quit)),
                    ..Default::default()
                }
                .into(),
            );
            items
        }
    }

    #[derive(Clone)]
    pub struct Handle(ksni::Handle<Tray>);

    impl Handle {
        pub async fn update(&self, model: Model) {
            self.0
                .update(move |t| {
                    if t.model != model {
                        t.model = model;
                    }
                })
                .await;
        }
    }

    /// Cria o ícone. Retorna None se a área de notificação não existe (sem StatusNotifierWatcher).
    pub async fn spawn(on_action: OnAction) -> Option<Handle> {
        let tray = Tray { model: Model::default(), on_action };
        match tray.spawn().await {
            Ok(h) => Some(Handle(h)),
            Err(e) => {
                log::info!("bandeja indisponível: {e}");
                None
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::cell::RefCell;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

    struct State {
        icon: TrayIcon,
        battery: MenuItem,
        modes: Vec<CheckMenuItem>,
        game: CheckMenuItem,
        audio: MenuItem,
        model: Model,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn icon(on: bool) -> Icon {
        Icon::from_rgba((if on { ON_32 } else { OFF_32 }).to_vec(), 32, 32).expect("ícone da bandeja")
    }

    /// Handle sem dados: o estado vive na thread da interface.
    #[derive(Clone)]
    pub struct Handle;

    impl Handle {
        /// Chame na thread da interface.
        pub fn update(&self, model: Model) {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let Some(st) = s.as_mut() else { return };
                if st.model == model {
                    return;
                }
                if st.model.connected != model.connected {
                    let _ = st.icon.set_icon(Some(icon(model.connected)));
                }
                let (title, description) = model.tooltip();
                let tip = if description.is_empty() { title } else { format!("{title}\n{description}") };
                let _ = st.icon.set_tooltip(Some(tip));
                st.battery.set_text(model.battery_line());
                for (i, m) in st.modes.iter().enumerate() {
                    m.set_enabled(model.connected);
                    m.set_checked(model.quick_mode == Some(i));
                }
                st.game.set_enabled(model.connected);
                st.game.set_checked(model.game_mode);
                st.audio.set_enabled(model.connected);
                st.model = model;
            });
        }
    }

    /// Cria o ícone na thread da interface (antes do laço de eventos).
    pub fn spawn(on_action: OnAction) -> Option<Handle> {
        let menu = Menu::new();
        let battery = MenuItem::new("Desconectado", false, None);
        let modes: Vec<CheckMenuItem> =
            QUICK_MODES.iter().map(|l| CheckMenuItem::new(*l, false, false, None)).collect();
        let game = CheckMenuItem::new("Modo jogo", false, false, None);
        let audio = MenuItem::new("Conectar áudio", false, None);
        let show = MenuItem::new("Abrir janela", true, None);
        let quit = MenuItem::new("Sair", true, None);
        let _ = menu.append(&battery);
        let _ = menu.append(&PredefinedMenuItem::separator());
        for m in &modes {
            let _ = menu.append(m);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&game);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&audio);
        let _ = menu.append(&show);
        let _ = menu.append(&quit);

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(format!("{APP_NAME} — desconectado"))
            .with_icon(icon(false))
            .build()
            .map_err(|e| log::info!("bandeja indisponível: {e}"))
            .ok()?;

        let ids: Vec<_> = modes.iter().map(|m| m.id().clone()).collect();
        let (game_id, audio_id, show_id, quit_id) =
            (game.id().clone(), audio.id().clone(), show.id().clone(), quit.id().clone());
        let act = on_action.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Some(i) = ids.iter().position(|id| *id == e.id) {
                act(Action::QuickMode(i));
            } else if e.id == game_id {
                let on = STATE.with(|s| s.borrow().as_ref().map(|s| !s.model.game_mode)).unwrap_or(true);
                act(Action::Game(on));
            } else if e.id == audio_id {
                act(Action::ConnectAudio);
            } else if e.id == show_id {
                act(Action::ShowWindow);
            } else if e.id == quit_id {
                act(Action::Quit);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = e {
                on_action(Action::ToggleWindow);
            }
        }));
        STATE.with(|s| {
            *s.borrow_mut() =
                Some(State { icon, battery, modes, game, audio, model: Model::default() })
        });
        Some(Handle)
    }
}

pub use imp::{spawn, Handle};
