"""Testes do HT05 com um cliente BLE falso."""

import pytest

from qcyctl import protocol as p
from qcyctl.device import HT05


class FakeChar:
    def __init__(self, uuid: str) -> None:
        self.uuid = uuid


class FakeClient:
    """Simula o fone: responde a escritas como o HT05 real."""

    def __init__(self) -> None:
        self.is_connected = True
        self.writes: list[tuple[str, bytes]] = []
        self.values = {
            p.CHAR_RESP: bytes.fromhex("ff1406010108037f7f0f0901020c0163100100160132"),
            p.CHAR_BATTERY: bytes([0x63, 0x64, 0x00]),
            p.CHAR_VERSION: bytes([4, 0, 0, 4, 0, 0]),
            p.CHAR_EQ: bytes.fromhex("ff03020100000000000102"),
            p.CHAR_KEYS: bytes.fromhex("01010201030204030507060b07000800"),
        }
        self.notify = None

    async def write_gatt_char(self, char, data, response=False):
        self.writes.append((char.uuid, bytes(data)))
        if char.uuid == p.CHAR_CMD:
            tlvs = p.parse_frame(bytes(data))
            for cmd_id, payload in tlvs:
                if cmd_id != p.Cmd.REQUEST:
                    # O fone confirma com o mesmo TLV na characteristic 1002.
                    self.notify(FakeChar(p.CHAR_RESP), bytearray(p.build_frame([(cmd_id, payload)])))
        elif char.uuid == p.CHAR_EQ:
            self.values[p.CHAR_EQ] = bytes(data) if data[0] == 0xFF else bytes([data[0]]) + bytes(10)
        elif char.uuid == p.CHAR_KEYS:
            keys = bytearray(self.values[p.CHAR_KEYS])
            for i in range(0, len(keys), 2):
                if keys[i] == data[0]:
                    keys[i + 1] = data[1]
            self.values[p.CHAR_KEYS] = bytes(keys)

    async def read_gatt_char(self, char):
        return bytearray(self.values[char.uuid])


@pytest.fixture
async def dev():
    d = HT05("C4:AC:60:11:22:34")
    fake = FakeClient()
    d._client = fake
    d._chars = {u: FakeChar(u) for u in fake.values | {p.CHAR_CMD: b""}}
    fake.notify = d._on_notify
    d.state.connected = True
    await d.refresh()
    return d


async def test_refresh_reads_state(dev):
    s = dev.state
    assert s.noise == p.NoiseMode(p.NoiseKind.ANC, "noisy", 3)
    assert s.game_mode is False
    assert s.battery.left.percent == 99
    assert s.version == ("4.0.0", "4.0.0")
    assert s.eq.gains == (3, 2, 1, 0, 0, 0, 0, 0, 1, 2)
    assert s.keys[6] == 11


async def test_set_noise_updates_from_notification(dev):
    seen = []
    dev.on_change(lambda s: seen.append(s.noise))
    await dev.set_noise(p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=4))
    assert dev._client.writes[-1] == (p.CHAR_CMD, bytes.fromhex("ff030c01a4"))
    assert dev.state.noise.level == 4
    assert seen


async def test_set_game_mode(dev):
    await dev.set_game_mode(True)
    assert dev.state.game_mode is True


async def test_set_eq_preset(dev):
    await dev.set_eq(p.Equalizer(2, ()))
    assert dev.state.eq.label() == "Graves"


async def test_set_key(dev):
    await dev.set_key(p.Gesture.TAP_LEFT, 4)
    assert dev.state.keys[1] == 4


async def test_battery_notification(dev):
    dev._on_notify(FakeChar(p.CHAR_BATTERY), bytearray([0x80 | 50, 40, 0]))
    assert dev.state.battery.left.charging


async def test_write_when_disconnected(dev):
    dev._client.is_connected = False
    with pytest.raises(ConnectionError):
        await dev.set_game_mode(True)
