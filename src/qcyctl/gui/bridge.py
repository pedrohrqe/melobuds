"""Ponte entre o Controller e a interface QML.

Expõe o estado do fone como propriedades Qt e os comandos como slots. As mudanças
aparecem na tela na hora (otimista); o eco do fone confirma ou corrige o valor.
"""

from __future__ import annotations

import shutil
import subprocess

from PySide6.QtCore import Property, QObject, QSettings, QTimer, Signal, Slot

from .. import autostart, classic
from .. import protocol as p
from ..device import DeviceState
from .controller import Controller

DEBOUNCE_MS = 350


def desktop_animations_enabled() -> bool:
    """Lê a configuração de animações do GNOME. Em outros ambientes, retorna True."""
    gs = shutil.which("gsettings")
    if not gs:
        return True
    try:
        out = subprocess.run(
            [gs, "get", "org.gnome.desktop.interface", "enable-animations"],
            capture_output=True, text=True, timeout=2, check=False,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return True
    return out != "false"


def _level(b: p.BatteryLevel | None) -> dict:
    if b is None:
        return {"percent": -1, "charging": False}
    return {"percent": -1 if b.percent is None else b.percent, "charging": b.charging}


class Backend(QObject):
    changed = Signal()
    statusChanged = Signal()
    reduceMotionChanged = Signal()
    audioOnlyChanged = Signal()
    bluetoothOnChanged = Signal()
    ignoreCaseChanged = Signal()
    autostartChanged = Signal()
    toast = Signal(str, str)          # texto, tipo ("error" | "info")

    def __init__(self, ctl: Controller, tray_available: bool = False) -> None:
        super().__init__()
        self.ctl = ctl
        self._s = DeviceState()
        self._status = "Procurando o fone…"
        self._tray = tray_available
        self._settings = QSettings("qcyctl", "qcyctl")
        saved = self._settings.value("reduce_motion")
        self._reduce = (str(saved).lower() == "true") if saved is not None else not desktop_animations_enabled()
        self._ignore_case = str(self._settings.value("ignore_case")).lower() == "true"

        self._noise_timer = self._timer(self._send_noise)
        self._eq_timer = self._timer(self._send_eq)
        self._pending_noise: p.NoiseMode | None = None
        self._pending_eq: p.Equalizer | None = None

        ctl.state_changed.connect(self.apply_state)
        ctl.status_changed.connect(self._on_status)
        ctl.error.connect(lambda msg: self.toast.emit(msg, "error"))
        ctl.audio_only_changed.connect(lambda _on: self.audioOnlyChanged.emit())
        ctl.bluetooth_changed.connect(lambda _on: self.bluetoothOnChanged.emit())

    def _timer(self, slot) -> QTimer:
        t = QTimer(self)
        t.setSingleShot(True)
        t.setInterval(DEBOUNCE_MS)
        t.timeout.connect(slot)
        return t

    # ---- entrada de estado ----

    @Slot(object)
    def apply_state(self, s: DeviceState) -> None:
        self._s = s
        self.changed.emit()

    def _on_status(self, text: str) -> None:
        self._status = text
        self.statusChanged.emit()

    # ---- propriedades ----

    def _get_connected(self) -> bool:
        return self._s.connected

    def _get_status(self) -> str:
        return self._status

    def _get_name(self) -> str:
        return (self._s.pair_name or self._s.name or "QCY HT05").strip()

    def _get_firmware(self) -> str:
        v = self._s.version
        if not v:
            return ""
        return v[0] if v[0] == v[1] else f"L {v[0]} · R {v[1]}"

    def _get_address(self) -> str:
        dev = self.ctl.device
        if dev is None:
            return ""
        addr = dev.address
        return getattr(addr, "address", addr) or ""

    def _get_battery(self) -> list:
        b = self._s.battery
        case = b.case if b and not self._ignore_case else None   # "Ignorar caixa": descarta o dado
        return [_level(b.left if b else None), _level(b.right if b else None), _level(case)]

    def _get_noise_kind(self) -> int:
        return int(self._s.noise.kind) if self._s.noise else -1

    def _get_anc_scene(self) -> str:
        n = self._s.noise
        return n.scene if n and n.kind is p.NoiseKind.ANC and n.scene else "noisy"

    def _get_anc_level(self) -> int:
        n = self._s.noise
        return n.level if n and n.kind is p.NoiseKind.ANC else 3

    def _get_trans_level(self) -> int:
        n = self._s.noise
        return n.level if n and n.kind is p.NoiseKind.TRANSPARENCY else 4

    def _get_noise_label(self) -> str:
        return self._s.noise.label() if self._s.noise else ""

    def _get_game(self) -> bool:
        return bool(self._s.game_mode)

    def _get_sleep(self) -> bool:
        return bool(self._s.sleep_mode)

    def _get_eq_preset(self) -> int:
        return self._s.eq.preset if self._s.eq else -1

    def _get_eq_gains(self) -> list:
        return list(self._s.eq.gains) if self._s.eq else [0] * len(p.EQ_BANDS)

    def _get_keys(self) -> list:
        return [self._s.keys.get(int(g), -1) for g in p.Gesture]

    def _get_reduce(self) -> bool:
        return self._reduce

    def _set_reduce(self, on: bool) -> None:
        if on != self._reduce:
            self._reduce = on
            self._settings.setValue("reduce_motion", on)
            self.reduceMotionChanged.emit()

    def _get_ignore_case(self) -> bool:
        return self._ignore_case

    def _set_ignore_case(self, on: bool) -> None:
        if on != self._ignore_case:
            self._ignore_case = on
            self._settings.setValue("ignore_case", on)
            self.ignoreCaseChanged.emit()
            self.changed.emit()   # a lista de bateria muda

    def _get_autostart(self) -> bool:
        return autostart.is_enabled()

    def _set_autostart(self, on: bool) -> None:
        if on == autostart.is_enabled():
            return
        try:
            autostart.set_enabled(on)
        except OSError as exc:
            self.toast.emit(f"O sistema não salvou a inicialização automática: {exc}", "error")
        self.autostartChanged.emit()

    def _get_tray(self) -> bool:
        return self._tray

    connected = Property(bool, _get_connected, notify=changed)
    status = Property(str, _get_status, notify=statusChanged)
    name = Property(str, _get_name, notify=changed)
    firmware = Property(str, _get_firmware, notify=changed)
    address = Property(str, _get_address, notify=changed)
    battery = Property("QVariantList", _get_battery, notify=changed)
    noiseKind = Property(int, _get_noise_kind, notify=changed)
    ancScene = Property(str, _get_anc_scene, notify=changed)
    ancLevel = Property(int, _get_anc_level, notify=changed)
    transLevel = Property(int, _get_trans_level, notify=changed)
    noiseLabel = Property(str, _get_noise_label, notify=changed)
    gameMode = Property(bool, _get_game, notify=changed)
    sleepMode = Property(bool, _get_sleep, notify=changed)
    eqPreset = Property(int, _get_eq_preset, notify=changed)
    eqGains = Property("QVariantList", _get_eq_gains, notify=changed)
    keys = Property("QVariantList", _get_keys, notify=changed)
    reduceMotion = Property(bool, _get_reduce, _set_reduce, notify=reduceMotionChanged)
    ignoreCase = Property(bool, _get_ignore_case, _set_ignore_case, notify=ignoreCaseChanged)
    autostart = Property(bool, _get_autostart, _set_autostart, notify=autostartChanged)
    appName = Property(str, lambda _s: autostart.APP_NAME, constant=True)
    creditUrl = Property(str, lambda _s: "https://github.com/pedrohrqe", constant=True)
    trayAvailable = Property(bool, _get_tray, constant=True)
    # Áudio conectado no sistema, mas o fone não anuncia o controle BLE.
    audioOnly = Property(bool, lambda self: bool(self.ctl.audio_only), notify=audioOnlyChanged)
    # Bluetooth do sistema. Desligado: a janela mostra o aviso e o app não procura o fone.
    bluetoothOn = Property(bool, lambda self: bool(self.ctl.bluetooth_on), notify=bluetoothOnChanged)
    # O app só consegue ligar o Bluetooth no Linux (BlueZ).
    canEnableBluetooth = Property(bool, lambda _s: classic.SUPPORTED, constant=True)

    # Tabelas fixas do protocolo.
    eqBands = Property("QVariantList", lambda _s: list(p.EQ_BANDS), constant=True)
    eqPresets = Property("QVariantList", lambda _s: [n for n, _ in p.EQ_PRESETS], constant=True)
    ancScenes = Property(
        "QVariantList",
        lambda _s: [{"key": k, "label": v[0]} for k, v in p.ANC_SCENES.items()],
        constant=True,
    )
    keyActions = Property(
        "QVariantList",
        lambda _s: [{"value": k, "label": v} for k, v in p.KEY_ACTIONS.items()],
        constant=True,
    )
    eqMin = Property(int, lambda _s: p.EQ_MIN_DB, constant=True)
    eqMax = Property(int, lambda _s: p.EQ_MAX_DB, constant=True)

    # ---- comandos ----

    def _optimistic(self, **fields) -> None:
        for k, v in fields.items():
            setattr(self._s, k, v)
        self.changed.emit()

    @Slot(int, str, int)
    def setNoise(self, kind: int, scene: str, level: int) -> None:
        k = p.NoiseKind(kind)
        if k is p.NoiseKind.ANC:
            level = 1 if scene == "wind" else max(1, min(p.ANC_LEVELS, level))
            mode = p.NoiseMode(k, scene, level)
        elif k is p.NoiseKind.TRANSPARENCY:
            mode = p.NoiseMode(k, level=max(0, min(6, level)))
        else:
            mode = p.NoiseMode(k)
        self._optimistic(noise=mode)
        self._pending_noise = mode
        self._noise_timer.start()

    def _send_noise(self) -> None:
        mode = self._pending_noise
        if mode is not None:
            self.ctl.run(lambda d: d.set_noise(mode))

    @Slot(int)
    def setEqPreset(self, index: int) -> None:
        eq = p.Equalizer(index, p.EQ_PRESETS[index][1])
        self._optimistic(eq=eq)
        self._pending_eq = eq
        self._eq_timer.start()

    @Slot(int, int)
    def setEqGain(self, band: int, db: int) -> None:
        gains = self._get_eq_gains()
        gains[band] = max(p.EQ_MIN_DB, min(p.EQ_MAX_DB, int(db)))
        eq = p.Equalizer.custom(gains)
        self._optimistic(eq=eq)
        self._pending_eq = eq
        self._eq_timer.start()

    def _send_eq(self) -> None:
        eq = self._pending_eq
        if eq is not None:
            self.ctl.run(lambda d: d.set_eq(eq))

    @Slot(int, int)
    def setKey(self, gesture: int, action: int) -> None:
        keys = dict(self._s.keys)
        keys[gesture] = action
        self._optimistic(keys=keys)
        self.ctl.run(lambda d: d.set_key(gesture, action))

    @Slot(bool)
    def setGame(self, on: bool) -> None:
        self._optimistic(game_mode=on)
        self.ctl.run(lambda d: d.set_game_mode(on))

    @Slot(bool)
    def setSleep(self, on: bool) -> None:
        self._optimistic(sleep_mode=on)
        self.ctl.run(lambda d: d.set_sleep_mode(on))

    @Slot(str, result=bool)
    def rename(self, name: str) -> bool:
        try:
            p.cmd_rename(name)
        except p.ProtocolError as exc:
            self.toast.emit(str(exc), "error")
            return False
        self._optimistic(pair_name=name)
        self.ctl.run(lambda d: d.rename(name))
        self.toast.emit("Nome enviado. Ele aparece nos outros aparelhos depois de reconectar.", "info")
        return True

    @Slot()
    def enableBluetooth(self) -> None:
        self.ctl.enable_bluetooth()

    @Slot()
    def wakeControl(self) -> None:
        self.ctl.wake()

    @Slot()
    def connectAudio(self) -> None:
        self.ctl.connect_audio()
