"""Testes da ponte QML: estado → propriedades e slots → comandos no fone."""

import asyncio

import pytest

pytest.importorskip("PySide6")

from PySide6.QtCore import QCoreApplication, QObject, QSettings, Signal

from qcyctl import protocol as p
from qcyctl.device import DeviceState
from qcyctl.gui import bridge


class FakeCtl(QObject):
    state_changed = Signal(object)
    status_changed = Signal(str)
    error = Signal(str)
    audio_only_changed = Signal(bool)
    bluetooth_changed = Signal(bool)

    def __init__(self):
        super().__init__()
        self.device = None
        self.calls = []
        self.audio = 0
        self.audio_only = False
        self.bluetooth_on = True
        self.enables = 0
        self.wakes = 0

    def run(self, factory):
        self.calls.append(factory)

    def enable_bluetooth(self):
        self.enables += 1

    def wake(self):
        self.wakes += 1

    def connect_audio(self):
        self.audio += 1


class FakeDevice:
    def __init__(self):
        self.log = []

    def __getattr__(self, name):
        async def call(*args):
            self.log.append((name, *args))
        return call


def executar(ctl):
    """Executa os comandos enfileirados e retorna as chamadas feitas no fone."""
    dev = FakeDevice()
    for f in ctl.calls:
        asyncio.run(f(dev))
    return dev.log


@pytest.fixture
def backend(tmp_path, monkeypatch):
    QCoreApplication.instance() or QCoreApplication([])
    QSettings.setPath(QSettings.NativeFormat, QSettings.UserScope, str(tmp_path))
    monkeypatch.setattr(bridge, "desktop_animations_enabled", lambda: True)
    ctl = FakeCtl()
    return bridge.Backend(ctl), ctl


def test_estado_vazio(backend):
    b, _ = backend
    assert b.connected is False
    assert b.noiseKind == -1
    assert b.eqPreset == -1
    assert b.eqGains == [0] * len(p.EQ_BANDS)
    assert [x["percent"] for x in b.battery] == [-1, -1, -1]
    assert b.reduceMotion is False


def test_estado_para_propriedades(backend):
    b, ctl = backend
    lvl = p.BatteryLevel
    st = DeviceState(
        connected=True,
        name="QCY HT05",
        pair_name="Pedrot",
        version=("4.0.0", "4.0.0"),
        battery=p.Battery(lvl(80, False), lvl(None, False), lvl(40, True)),
        noise=p.NoiseMode(p.NoiseKind.ANC, "indoor", 2),
        game_mode=True,
        eq=p.Equalizer(1, p.EQ_PRESETS[1][1]),
        keys={int(p.Gesture.TAP_LEFT): 5},
    )
    sinais = []
    b.changed.connect(lambda: sinais.append(1))
    ctl.state_changed.emit(st)
    assert sinais
    assert b.connected and b.name == "Pedrot" and b.firmware == "4.0.0"
    assert b.battery == [
        {"percent": 80, "charging": False},
        {"percent": -1, "charging": False},
        {"percent": 40, "charging": True},
    ]
    assert (b.noiseKind, b.ancScene, b.ancLevel) == (1, "indoor", 2)
    assert b.gameMode is True and b.sleepMode is False
    assert b.eqPreset == 1
    assert b.keys[0] == 5 and b.keys[1] == -1


def test_firmware_diferente(backend):
    b, ctl = backend
    ctl.state_changed.emit(DeviceState(version=("1.0", "1.1")))
    assert b.firmware == "L 1.0 · R 1.1"


def test_set_noise_otimista_e_debounce(backend):
    b, ctl = backend
    b.setNoise(1, "wind", 3)
    assert b.noiseKind == 1 and b.ancLevel == 1          # vento usa nível 1
    b.setNoise(2, "", 9)
    assert b.transLevel == 6                              # limitado a 6
    assert ctl.calls == []                                # espera o debounce
    b._send_noise()
    assert executar(ctl) == [("set_noise", p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=6))]


def test_eq_ganho_vira_personalizado(backend):
    b, ctl = backend
    b.setEqPreset(2)
    b.setEqGain(0, 99)
    assert b.eqPreset == p.EQ_CUSTOM
    assert b.eqGains[0] == p.EQ_MAX_DB
    b._send_eq()
    (nome, eq), = executar(ctl)
    assert nome == "set_eq" and eq.preset == p.EQ_CUSTOM


def test_comandos_diretos(backend):
    b, ctl = backend
    b.setKey(3, 7)
    b.setGame(True)
    b.setSleep(False)
    b.connectAudio()
    assert b.keys[2] == 7 and b.gameMode is True
    assert executar(ctl) == [("set_key", 3, 7), ("set_game_mode", True), ("set_sleep_mode", False)]
    assert ctl.audio == 1


def test_rename(backend):
    b, ctl = backend
    avisos = []
    b.toast.connect(lambda t, k: avisos.append(k))
    assert b.rename("x" * 200) is False
    assert b.rename("Fone") is True
    assert b.name == "Fone"
    assert avisos == ["error", "info"]
    assert executar(ctl) == [("rename", "Fone")]


def test_reduce_motion_salvo(backend):
    b, _ = backend
    sinais = []
    b.reduceMotionChanged.connect(lambda: sinais.append(1))
    b.reduceMotion = True
    assert b.reduceMotion is True and sinais == [1]
    assert str(QSettings("qcyctl", "qcyctl").value("reduce_motion")).lower() == "true"


def test_audio_only(backend):
    b, ctl = backend
    sinais = []
    b.audioOnlyChanged.connect(lambda: sinais.append(1))
    assert b.audioOnly is False
    ctl.audio_only = True
    ctl.audio_only_changed.emit(True)
    assert b.audioOnly is True and sinais == [1]
    b.wakeControl()
    assert ctl.wakes == 1


def test_fones_na_caixa():
    from qcyctl.gui.controller import _in_case
    lvl = p.BatteryLevel
    assert _in_case(p.Battery(lvl(85, True), lvl(89, True), lvl(50, False)))
    assert _in_case(p.Battery(lvl(None, False), lvl(89, True), lvl(50, False)))
    assert not _in_case(p.Battery(lvl(85, True), lvl(89, False), lvl(50, False)))
    assert not _in_case(p.Battery(lvl(85, False), lvl(89, False), lvl(50, False)))
    assert not _in_case(None)


def test_bluetooth_desligado(backend):
    b, ctl = backend
    sinais = []
    b.bluetoothOnChanged.connect(lambda: sinais.append(1))
    assert b.bluetoothOn is True
    ctl.bluetooth_on = False
    ctl.bluetooth_changed.emit(False)
    assert b.bluetoothOn is False and sinais == [1]
    b.enableBluetooth()
    assert ctl.enables == 1


def test_ignorar_caixa(backend):
    b, ctl = backend
    lvl = p.BatteryLevel
    ctl.state_changed.emit(DeviceState(connected=True, battery=p.Battery(lvl(80, False), lvl(70, False), lvl(40, True))))
    assert b.battery[2] == {"percent": 40, "charging": True}
    b.ignoreCase = True
    assert b.ignoreCase is True
    assert b.battery[2] == {"percent": -1, "charging": False}
    assert b.battery[0]["percent"] == 80
    assert str(QSettings("qcyctl", "qcyctl").value("ignore_case")).lower() == "true"


def test_autostart(backend, tmp_path, monkeypatch):
    from qcyctl import autostart
    if autostart.sys.platform == "win32":
        pytest.skip("teste do arquivo .desktop")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "cfg"))
    b, _ = backend
    assert b.autostart is False
    b.autostart = True
    arquivo = tmp_path / "cfg" / "autostart" / "melobuds.desktop"
    texto = arquivo.read_text(encoding="utf-8")
    assert "Name=MeloBuds by @pedrohrqe" in texto and "--hidden" in texto
    assert b.autostart is True
    b.autostart = False
    assert not arquivo.exists()
