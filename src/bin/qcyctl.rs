//! CLI do qcyctl: descoberta e testes de protocolo do QCY HT05.

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use btleplug::api::{Central, CharPropFlags, Characteristic, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Peripheral};
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use futures::StreamExt;
use melobuds::device::{self, DeviceState, HT05};
use melobuds::protocol as p;
use melobuds::{classic, dualboot};

const NOT_FOUND: &str = "nenhum fone QCY encontrado. Tire os fones da caixa e tente de novo.";

#[derive(Parser)]
#[command(name = "qcyctl", about = "Controle do fone QCY HT05 pela linha de comando")]
struct Cli {
    /// mostra mensagens de depuração
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// mostra bateria, modos, EQ e toques
    Status {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
    },
    /// modo de ruído: off | anc [indoor|commute|noisy|wind] [1-3] | transparency [0-6]
    Noise {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
        #[arg(required = true, num_args = 1..)]
        mode: Vec<String>,
    },
    /// EQ: preset por nome ou --gains com 10 valores (-8..8)
    #[command(group(ArgGroup::new("fonte").required(true).args(["preset", "gains"])))]
    Eq {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
        preset: Option<String>,
        /// ex.: 3,2,1,0,0,0,0,0,1,2
        #[arg(long, allow_hyphen_values = true)]
        gains: Option<String>,
    },
    /// modo jogo (baixa latência)
    Game {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
        state: OnOff,
    },
    /// muda o nome Bluetooth do fone (até 32 bytes)
    Rename {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
        name: String,
    },
    /// pareia (se preciso) e conecta o áudio do fone (Linux)
    Audio {
        /// MAC clássico (padrão: lido do anúncio BLE)
        #[arg(long)]
        classic: Option<String>,
    },
    /// copia a chave de pareamento do Windows para o Linux (sudo)
    Dualboot {
        /// ponto de montagem da partição Windows
        #[arg(long)]
        win: Option<String>,
        /// MAC clássico do fone (padrão: todos)
        #[arg(long)]
        device: Option<String>,
        /// só compara, não altera
        #[arg(long)]
        dry_run: bool,
    },
    /// remapeia toque: gesto 1-6, ação (0-7, 11)
    Key {
        /// endereço BLE (padrão: o primeiro QCY encontrado)
        #[arg(short, long)]
        address: Option<String>,
        #[arg(value_parser = clap::value_parser!(u8).range(1..=6))]
        gesture: u8,
        #[arg(value_parser = parse_action)]
        action: u8,
    },
    /// lista fones QCY (--all: todos os BLE)
    Scan {
        #[arg(long, default_value_t = 8.0)]
        timeout: f64,
        /// mostra todos, não só QCY
        #[arg(long)]
        all: bool,
    },
    /// lista serviços GATT de um dispositivo
    Probe {
        address: String,
        #[arg(long, default_value_t = 15.0)]
        timeout: f64,
    },
    /// envia bytes crus e mostra notificações
    Raw {
        address: String,
        /// UUID da characteristic de escrita
        #[arg(long, required = true)]
        write: String,
        /// UUID para notificações
        #[arg(long)]
        notify: Vec<String>,
        #[arg(long)]
        no_response: bool,
        #[arg(long, default_value_t = 1.5)]
        wait: f64,
        #[arg(long, default_value_t = 15.0)]
        timeout: f64,
        /// pacotes em hex, ex.: aa0102
        #[arg(required = true, num_args = 1..)]
        hex: Vec<String>,
    },
    /// mostra notificações por um tempo
    Monitor {
        address: String,
        #[arg(long, default_value_t = 60.0)]
        seconds: f64,
        #[arg(long)]
        all_services: bool,
        #[arg(long, default_value_t = 15.0)]
        timeout: f64,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum OnOff {
    On,
    Off,
}

fn parse_action(s: &str) -> Result<u8, String> {
    let v: u8 = s.parse().map_err(|_| format!("'{s}' não é um número"))?;
    if p::KEY_ACTIONS.iter().any(|a| a.0 == v) {
        Ok(v)
    } else {
        let ok: Vec<String> = p::KEY_ACTIONS.iter().map(|a| a.0.to_string()).collect();
        Err(format!("ação inválida: {v} (escolha entre {})", ok.join(", ")))
    }
}

fn secs(v: f64) -> Duration {
    Duration::from_secs_f64(v.max(0.0))
}

/// Representação de texto no estilo `repr()` do Python (aspas simples).
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Representação de bytes no estilo `repr()` do Python (`b'...'`).
fn py_bytes_repr(data: &[u8]) -> String {
    let mut out = String::from("b'");
    for &b in data {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push('\'');
    out
}

fn on_off(v: bool) -> &'static str {
    if v { "ligado" } else { "desligado" }
}

// --- conexão ---------------------------------------------------------------

/// Procura um periférico pelo endereço, escaneando até `timeout`.
async fn find_peripheral(adapter: &Adapter, address: &str, timeout: Duration) -> Result<Peripheral> {
    adapter.start_scan(ScanFilter::default()).await.context("falha ao iniciar o scan")?;
    let deadline = tokio::time::Instant::now() + timeout;
    let want = address.to_uppercase();
    let found = loop {
        let mut hit = None;
        for per in adapter.peripherals().await? {
            if per.address().to_string().to_uppercase() == want {
                hit = Some(per);
                break;
            }
        }
        if hit.is_some() || tokio::time::Instant::now() >= deadline {
            break hit;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    let _ = adapter.stop_scan().await;
    found.ok_or_else(|| anyhow!("dispositivo {address} não encontrado"))
}

/// Conecta num periférico e descobre os serviços GATT (probe, raw, monitor).
async fn open_gatt(address: &str, timeout: f64) -> Result<Peripheral> {
    let adapter = device::adapter().await?;
    let per = find_peripheral(&adapter, address, secs(timeout)).await?;
    tokio::time::timeout(secs(timeout), per.connect()).await.map_err(|_| anyhow!("tempo esgotado ao conectar"))??;
    per.discover_services().await?;
    Ok(per)
}

async fn open(address: Option<String>) -> Result<std::sync::Arc<HT05>> {
    let adapter = device::adapter().await?;
    let per = match address {
        Some(a) => find_peripheral(&adapter, &a, Duration::from_secs(10)).await?,
        None => match device::find_first_with_adv(&adapter, Duration::from_secs(10)).await? {
            Some(f) => f.peripheral,
            None => bail!(NOT_FOUND),
        },
    };
    HT05::connect(&adapter, per, Duration::from_secs(15)).await
}

/// Espera `d`; retorna `true` se o usuário apertou Ctrl+C antes.
async fn sleep_or_interrupt(d: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(d) => false,
        _ = tokio::signal::ctrl_c() => true,
    }
}

// --- impressão -------------------------------------------------------------

fn print_state(s: &DeviceState) {
    match &s.pair_name {
        Some(n) => println!("Nome:       {}", py_repr(n)),
        None => println!("Nome:       ?"),
    }
    match &s.version {
        Some((l, r)) => println!("Firmware:   L {l}  R {r}"),
        None => println!("Firmware:   ?"),
    }
    if let Some(b) = &s.battery {
        println!("Bateria:    L {}  R {}  caixa {}", b.left.label(), b.right.label(), b.case.label());
    }
    println!("Ruído:      {}", s.noise.map_or("?".to_string(), |n| n.label()));
    println!("Modo jogo:  {}", on_off(s.game_mode == Some(true)));
    println!("Modo sono:  {}", on_off(s.sleep_mode == Some(true)));
    if let Some(eq) = &s.eq {
        println!("EQ:         {}  {:?}", eq.label(), eq.gains);
    }
    for g in p::Gesture::ALL {
        if let Some(&action) = s.keys.get(&(g as u8)) {
            let (gesture, side) = g.label();
            let label = p::key_action_label(action).map_or(format!("{action:#x}"), str::to_string);
            println!("Toque:      {gesture} {side}: {label}");
        }
    }
}

fn parse_noise(text: &str) -> Result<p::NoiseMode> {
    let lower = text.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    let num = |s: &str| s.parse::<u8>().map_err(|_| anyhow!("número inválido: {s}"));
    match parts.first().copied() {
        Some("off" | "normal") => Ok(p::NoiseMode::normal()),
        Some("transparency" | "trans" | "t") => {
            Ok(p::NoiseMode::transparency(parts.get(1).map_or(Ok(4), |s| num(s))?))
        }
        Some("anc") => {
            let scene = parts.get(1).copied().unwrap_or("noisy");
            let level = match parts.get(2) {
                Some(s) => num(s)?,
                None if scene == "wind" => 1,
                None => 3,
            };
            let name = p::ANC_SCENES
                .iter()
                .find(|s| s.0 == scene)
                .map(|s| s.0)
                .ok_or_else(|| anyhow!("cena ANC inválida: {scene}"))?;
            Ok(p::NoiseMode::anc(name, level))
        }
        _ => bail!("modo inválido: {text}"),
    }
}

// --- comandos --------------------------------------------------------------

async fn cmd_scan(timeout: f64, all: bool) -> Result<i32> {
    let adapter = device::adapter().await?;
    if !all {
        for d in device::discover(&adapter, secs(timeout), false).await? {
            let ad = &d.advertisement;
            let b = &ad.battery;
            println!(
                "{}  modelo=0x{:04x}  clássico={}  bateria L={} R={} caixa={}",
                d.address,
                ad.vendor_id,
                ad.classic_mac,
                b.left.label(),
                b.right.label(),
                b.case.label()
            );
        }
        return Ok(0);
    }
    adapter.start_scan(ScanFilter::default()).await?;
    tokio::time::sleep(secs(timeout)).await;
    let mut found = Vec::new();
    for per in adapter.peripherals().await? {
        let Ok(Some(props)) = per.properties().await else { continue };
        if props.rssi.is_some() {
            found.push(props);
        }
    }
    let _ = adapter.stop_scan().await;
    found.sort_by_key(|p| std::cmp::Reverse(p.rssi));
    for props in found {
        let name = props.local_name.clone().unwrap_or_else(|| "?".into());
        println!("{}  rssi={:4}  {}", props.address, props.rssi.unwrap_or(0), name);
        let mut ids: Vec<_> = props.manufacturer_data.iter().collect();
        ids.sort();
        for (cid, data) in ids {
            let h: String = data.iter().map(|b| format!("{b:02x}")).collect();
            println!("    manufacturer 0x{cid:04x}: {h}");
        }
    }
    Ok(0)
}

async fn cmd_status(address: Option<String>) -> Result<i32> {
    let dev = open(address).await?;
    print_state(&dev.state());
    dev.disconnect().await?;
    Ok(0)
}

async fn cmd_noise(address: Option<String>, mode: Vec<String>) -> Result<i32> {
    let mode = parse_noise(&mode.join(" "))?;
    let dev = open(address).await?;
    let res = async {
        dev.set_noise(&mode).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("Ruído: {}", dev.state().noise.map_or("?".to_string(), |n| n.label()));
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = dev.disconnect().await;
    res.map(|_| 0)
}

async fn cmd_eq(address: Option<String>, preset: Option<String>, gains: Option<String>) -> Result<i32> {
    let eq = if let Some(g) = gains {
        let values = g
            .split(',')
            .map(|x| x.trim().parse::<i32>().map_err(|_| anyhow!("ganho inválido: {x}")))
            .collect::<Result<Vec<_>>>()?;
        p::Equalizer::custom(&values)
    } else {
        let wanted = preset.unwrap_or_default().to_lowercase();
        let names: Vec<String> = p::EQ_PRESETS.iter().map(|(n, _)| n.to_lowercase()).collect();
        let Some(idx) = names.iter().position(|n| *n == wanted) else {
            let opts: Vec<&str> = p::EQ_PRESETS.iter().map(|(n, _)| *n).collect();
            bail!("preset inválido. Opções: {}", opts.join(", "));
        };
        p::Equalizer { preset: idx as u8, gains: Vec::new() }
    };
    let dev = open(address).await?;
    let res = async {
        dev.set_eq(&eq).await?;
        if let Some(e) = dev.state().eq {
            println!("EQ: {}  {:?}", e.label(), e.gains);
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = dev.disconnect().await;
    res.map(|_| 0)
}

async fn cmd_game(address: Option<String>, state: OnOff) -> Result<i32> {
    let dev = open(address).await?;
    let res = async {
        dev.set_game_mode(matches!(state, OnOff::On)).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("Modo jogo: {}", on_off(dev.state().game_mode == Some(true)));
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = dev.disconnect().await;
    res.map(|_| 0)
}

async fn cmd_rename(address: Option<String>, name: String) -> Result<i32> {
    p::cmd_rename(&name)?; // valida antes de conectar
    let dev = open(address).await?;
    let res = async {
        dev.rename(&name).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        match dev.state().pair_name {
            Some(n) => println!("Nome: {}", py_repr(&n)),
            None => println!("Nome: ?"),
        }
        println!("O novo nome aparece depois que o fone reconectar (guarde na caixa e tire de novo).");
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = dev.disconnect().await;
    res.map(|_| 0)
}

async fn cmd_key(address: Option<String>, gesture: u8, action: u8) -> Result<i32> {
    let dev = open(address).await?;
    let res = async {
        dev.set_key(gesture, action).await?;
        print_state(&dev.state());
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = dev.disconnect().await;
    res.map(|_| 0)
}

fn props_label(f: CharPropFlags) -> String {
    let names = [
        (CharPropFlags::BROADCAST, "broadcast"),
        (CharPropFlags::READ, "read"),
        (CharPropFlags::WRITE_WITHOUT_RESPONSE, "write-without-response"),
        (CharPropFlags::WRITE, "write"),
        (CharPropFlags::NOTIFY, "notify"),
        (CharPropFlags::INDICATE, "indicate"),
        (CharPropFlags::AUTHENTICATED_SIGNED_WRITES, "authenticated-signed-writes"),
        (CharPropFlags::EXTENDED_PROPERTIES, "extended-properties"),
    ];
    names.iter().filter(|(flag, _)| f.contains(*flag)).map(|(_, n)| *n).collect::<Vec<_>>().join(",")
}

async fn cmd_probe(address: String, timeout: f64) -> Result<i32> {
    let per = open_gatt(&address, timeout).await?;
    for service in per.services() {
        println!("[service] {}", service.uuid);
        for ch in &service.characteristics {
            let mut line = format!("   [char] {}  ({})", ch.uuid, props_label(ch.properties));
            if ch.properties.contains(CharPropFlags::READ) {
                match per.read(ch).await {
                    Ok(v) => line += &format!("  = {}  {}", v.iter().map(|b| format!("{b:02x}")).collect::<String>(), py_bytes_repr(&v)),
                    Err(e) => line += &format!("  (erro de leitura: {e})"),
                }
            }
            println!("{line}");
        }
    }
    let _ = per.disconnect().await;
    Ok(0)
}

/// Acha uma characteristic pelo UUID (completo ou de 16 bits, como "1001").
fn find_char(per: &Peripheral, text: &str) -> Result<Characteristic> {
    if text.to_lowercase().starts_with("0x") {
        bail!("referência por handle (0x....) não é suportada; use o UUID da characteristic");
    }
    let t = text.to_lowercase();
    let chars: BTreeSet<Characteristic> = per.characteristics();
    chars
        .into_iter()
        .find(|c| {
            let u = c.uuid.to_string();
            u == t || (t.len() == 4 && u[4..8] == t && u.starts_with("0000") && u.ends_with("-0000-1000-8000-00805f9b34fb"))
        })
        .ok_or_else(|| anyhow!("characteristic {text} não encontrada"))
}

fn short_uuid(u: &uuid::Uuid) -> String {
    u.to_string()[4..8].to_string()
}

/// Imprime as notificações recebidas, uma por linha.
async fn spawn_notify_printer(per: &Peripheral) -> Result<tokio::task::JoinHandle<()>> {
    let mut notes = per.notifications().await?;
    Ok(tokio::spawn(async move {
        while let Some(n) = notes.next().await {
            println!("<- {}: {}", short_uuid(&n.uuid), device::hex(&n.value));
        }
    }))
}

fn parse_hex(text: &str) -> Result<Vec<u8>> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        bail!("hex inválido: {text}");
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).map_err(|_| anyhow!("hex inválido: {text}")))
        .collect()
}

async fn cmd_raw(
    address: String,
    write: String,
    notify: Vec<String>,
    no_response: bool,
    wait: f64,
    timeout: f64,
    hex: Vec<String>,
) -> Result<i32> {
    // Valida os pacotes antes de conectar.
    let payloads = hex.iter().map(|h| parse_hex(h)).collect::<Result<Vec<_>>>()?;
    let per = open_gatt(&address, timeout).await?;
    let res = async {
        let printer = spawn_notify_printer(&per).await?;
        for u in &notify {
            per.subscribe(&find_char(&per, u)?).await?;
        }
        let target = find_char(&per, &write)?;
        let kind = if no_response { WriteType::WithoutResponse } else { WriteType::WithResponse };
        let mut code = 0;
        for data in payloads {
            println!("-> {}: {}", write, device::hex(&data));
            per.write(&target, &data, kind).await?;
            if sleep_or_interrupt(secs(wait)).await {
                code = 130;
                break;
            }
        }
        printer.abort();
        Ok::<_, anyhow::Error>(code)
    }
    .await;
    let _ = per.disconnect().await;
    res
}

async fn cmd_monitor(address: String, seconds: f64, all_services: bool, timeout: f64) -> Result<i32> {
    let per = open_gatt(&address, timeout).await?;
    let res = async {
        let printer = spawn_notify_printer(&per).await?;
        for service in per.services() {
            if all_services || service.uuid.to_string().starts_with("0000a001") {
                for ch in &service.characteristics {
                    if ch.properties.contains(CharPropFlags::NOTIFY) {
                        per.subscribe(ch).await?;
                    }
                }
            }
        }
        println!("monitorando por {seconds:.0} s...");
        let code = if sleep_or_interrupt(secs(seconds)).await { 130 } else { 0 };
        printer.abort();
        Ok::<_, anyhow::Error>(code)
    }
    .await;
    let _ = per.disconnect().await;
    res
}

async fn cmd_dualboot(win: Option<String>, device: Option<String>, dry_run: bool) -> Result<i32> {
    dualboot::run(win.as_deref(), device.as_deref(), dry_run).map_err(|e| anyhow!("erro: {e}"))
}

async fn cmd_audio(classic_mac: Option<String>) -> Result<i32> {
    let mac = match classic_mac {
        Some(m) => m,
        None => {
            let adapter = device::adapter().await?;
            match device::find_first_with_adv(&adapter, Duration::from_secs(15)).await? {
                Some(f) => f.advertisement.classic_mac,
                None => bail!(NOT_FOUND),
            }
        }
    };
    println!("Áudio {mac}: {}", classic::ensure_connected(&mac).await);
    Ok(0)
}

async fn run(cmd: Cmd) -> Result<i32> {
    match cmd {
        Cmd::Status { address } => cmd_status(address).await,
        Cmd::Noise { address, mode } => cmd_noise(address, mode).await,
        Cmd::Eq { address, preset, gains } => cmd_eq(address, preset, gains).await,
        Cmd::Game { address, state } => cmd_game(address, state).await,
        Cmd::Rename { address, name } => cmd_rename(address, name).await,
        Cmd::Audio { classic } => cmd_audio(classic).await,
        Cmd::Dualboot { win, device, dry_run } => cmd_dualboot(win, device, dry_run).await,
        Cmd::Key { address, gesture, action } => cmd_key(address, gesture, action).await,
        Cmd::Scan { timeout, all } => cmd_scan(timeout, all).await,
        Cmd::Probe { address, timeout } => cmd_probe(address, timeout).await,
        Cmd::Raw { address, write, notify, no_response, wait, timeout, hex } => {
            cmd_raw(address, write, notify, no_response, wait, timeout, hex).await
        }
        Cmd::Monitor { address, seconds, all_services, timeout } => cmd_monitor(address, seconds, all_services, timeout).await,
    }
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(if cli.debug { "debug" } else { "warn" })).init();
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    };
    let code = match rt.block_on(run(cli.cmd)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    };
    std::process::exit(code);
}
