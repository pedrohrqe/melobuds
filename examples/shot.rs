//! Renderiza MainWindow com o renderizador de software em PNGs (sem abrir janela).
//! Uso: cargo run --example shot -- <pasta-de-saída> [estado]
//! Serve para comparar a interface Slint com as capturas do app QML.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ModelRc, PhysicalSize, Rgb8Pixel, SharedString, VecModel};

slint::include_modules!();

thread_local! {
    static NOW: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

struct Shot {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for Shot {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        NOW.with(|n| n.get())
    }
}

const W: u32 = 940;

fn bat(p: i32, c: bool) -> Battery {
    Battery { percent: p, charging: c }
}

fn set_battery(ui: &MainWindow, l: Battery, r: Battery, c: Battery) {
    ui.global::<Backend>().set_battery(ModelRc::new(VecModel::from(vec![l, r, c])));
}

/// Avança o relógio em passos de 16 ms, renderizando cada quadro.
fn run(ui: &MainWindow, win: &Rc<MinimalSoftwareWindow>, ms: u64, buf: &mut Vec<Rgb8Pixel>) -> (u32, u32) {
    let mut size = (W, 700);
    for _ in 0..=(ms / 16) {
        NOW.with(|n| n.set(n.get() + Duration::from_millis(16)));
        slint::platform::update_timers_and_animations();
        let h = ui.get_fit_height().ceil() as u32;
        size = (W, h.max(100));
        win.set_size(PhysicalSize::new(size.0, size.1));
        buf.resize((size.0 * size.1) as usize, Rgb8Pixel::default());
        win.draw_if_needed(|r| {
            r.render(buf, size.0 as usize);
        });
    }
    size
}

fn save(path: &str, buf: &[Rgb8Pixel], size: (u32, u32)) {
    let mut raw = Vec::with_capacity(buf.len() * 3);
    for p in buf {
        raw.extend_from_slice(&[p.r, p.g, p.b]);
    }
    image::save_buffer(path, &raw, size.0, size.1, image::ColorType::Rgb8).expect("falha ao gravar PNG");
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let only = std::env::args().nth(2);
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Shot { window: window.clone() })).unwrap();

    let ui = MainWindow::new().unwrap();
    ui.set_live(true);
    ui.show().unwrap();
    let be = ui.global::<Backend>();
    let mut buf: Vec<Rgb8Pixel> = Vec::new();

    let mut shot = |name: &str, ui: &MainWindow, ms: u64| {
        let size = run(ui, &window, ms, &mut buf);
        if only.as_deref().map_or(true, |o| name.starts_with(o)) {
            save(&format!("{out}/{name}-{W}.png"), &buf, size);
        }
    };

    // 01: procurando (desconectado, Bluetooth ligado).
    be.set_status(SharedString::from("Procurando o fone…"));
    shot("01-procurando", &ui, 900);

    // 01b: Bluetooth desligado.
    be.set_bluetooth_on(false);
    be.set_can_enable_bluetooth(true);
    be.set_status(SharedString::from("Bluetooth desligado"));
    shot("01b-bt-off", &ui, 1500);

    // 01c: Bluetooth ligou de novo.
    be.set_bluetooth_on(true);
    be.set_status(SharedString::from("Procurando o fone…"));
    shot("01c-bt-on", &ui, 1500);

    // 02: conectado, ANC nível 2, cena interna.
    be.set_connected(true);
    be.set_name("Pedrot".into());
    be.set_status(SharedString::from("Conectado (controle e áudio)"));
    be.set_firmware("4.0.0".into());
    set_battery(&ui, bat(98, false), bat(74, false), bat(62, false));
    be.set_noise_kind(1);
    be.set_anc_scene("indoor".into());
    be.set_anc_level(2);
    be.set_noise_label("ANC Interno 2".into());
    be.set_eq_preset(1);
    be.set_eq_gains(ModelRc::new(VecModel::from(vec![-6, 5, -3, -2, 5, 4, -4, -3, 6, 4])));
    be.set_keys(ModelRc::new(VecModel::from(vec![1, 1, 2, 3, 7, 11])));
    shot("02-anc", &ui, 2600);

    // 03: transparência nível 4.
    be.set_noise_kind(2);
    be.set_trans_level(4);
    be.set_noise_label("Transparência 4".into());
    shot("03-transp", &ui, 1200);

    // 04: fone esquerdo carregando com bateria baixa (12 %), direito 27 %, caixa sem leitura.
    set_battery(&ui, bat(12, true), bat(27, false), bat(-1, false));
    shot("04-carga-baixa", &ui, 1500);

    // 05: aba Equalizador (preset Pop); 05b: ganhos personalizados.
    set_battery(&ui, bat(98, false), bat(74, false), bat(62, false));
    ui.set_tab_index(0);
    shot("05-eq", &ui, 1000);
    be.set_noise_kind(1);
    be.set_noise_label("ANC Interno 2".into());
    be.set_eq_preset(255);
    be.set_eq_gains(ModelRc::new(VecModel::from(vec![3, 2, 0, -2, -4, 0, 2, 4, 6, 5])));
    shot("05b-eq-custom", &ui, 1500);

    // 06: aba Toques.
    ui.set_tab_index(1);
    shot("06-toques", &ui, 1000);

    // 07: aba Dispositivo.
    ui.set_tab_index(2);
    shot("07-dispositivo", &ui, 1000);

    // 08: reduzir animações, modo normal.
    be.set_reduce_motion(true);
    ui.set_tab_index(0);
    be.set_noise_kind(0);
    be.set_noise_label("Normal".into());
    shot("08-reduzido", &ui, 800);
    // Com "reduzir animações", nenhum laço infinito pede quadros.
    println!("reduzido, live: animações ativas = {}", window.has_active_animations());

    // 09: aviso de erro (toast).
    be.set_reduce_motion(false);
    be.set_noise_kind(1);
    be.set_noise_label("ANC Interno 2".into());
    be.invoke_toast("O sistema não salvou a inicialização automática: permissão negada.".into(), "error".into());
    shot("09-toast-erro", &ui, 900);
    println!("conectado, live, ANC: animações ativas = {}", window.has_active_animations());

    // Fora de foco (live = false), os laços param e nenhum quadro é pedido.
    ui.set_live(false);
    run(&ui, &window, 700, &mut buf);
    println!("sem foco: animações ativas = {}", window.has_active_animations());
}
