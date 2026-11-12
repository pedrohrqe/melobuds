"""Testes do protocolo com pacotes reais capturados do HT05 (firmware 4.0.0)."""

import pytest

from qcyctl import protocol as p

# Pacotes reais lidos do fone em 2026-11-12.
STATE_FRAME = bytes.fromhex("ff1406010108037f7f0f0901020c0163100100160132")
NAME_FRAME = bytes.fromhex("ff221820" + "506564726f7420" + "00" * 25)  # "Pedrot " + zeros
ADV_PAYLOAD = bytes.fromhex("4d41ae806a646400000000ac846033221100000000000000")


def test_parse_state_frame():
    tlvs = dict(p.parse_frame(STATE_FRAME))
    assert tlvs[p.Cmd.NOISE_MODE] == b"\x63"
    assert tlvs[p.Cmd.GAME_MODE] == b"\x02"
    assert tlvs[p.Cmd.BALANCE] == b"\x32"
    assert tlvs[p.Cmd.VOLUME] == bytes([0x7F, 0x7F, 0x0F])


@pytest.mark.parametrize("bad", [b"", b"\xff", b"\xfe\x01\x00", b"\xff\x05\x0c\x01\x02"])
def test_parse_frame_rejects_invalid(bad):
    with pytest.raises(p.ProtocolError):
        p.parse_frame(bad)


def test_build_frame_roundtrip():
    frame = p.build_frame([(0x0C, b"\x02"), (0x09, b"\x01")])
    assert frame == bytes.fromhex("ff060c0102090101")
    assert p.parse_frame(frame) == [(0x0C, b"\x02"), (0x09, b"\x01")]


def test_commands_match_official_app():
    # Valores do painel oficial e do teste no fone.
    assert p.cmd_set(p.Cmd.GAME_MODE, p.ON) == bytes.fromhex("ff03090101")
    assert p.cmd_set(p.Cmd.GAME_MODE, p.OFF) == bytes.fromhex("ff03090102")
    assert p.cmd_set(p.Cmd.SLEEP_MODE, p.ON) == bytes.fromhex("ff03100101")
    assert p.cmd_request(p.Cmd.NOISE_MODE) == bytes.fromhex("ff03fe010c")


@pytest.mark.parametrize(
    ("value", "mode"),
    [
        (0x02, p.NoiseMode(p.NoiseKind.NORMAL)),
        (0x41, p.NoiseMode(p.NoiseKind.ANC, "indoor", 1)),
        (0x53, p.NoiseMode(p.NoiseKind.ANC, "commute", 3)),
        (0x63, p.NoiseMode(p.NoiseKind.ANC, "noisy", 3)),
        (0x71, p.NoiseMode(p.NoiseKind.ANC, "wind", 1)),
        (0xA0, p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=0)),
        (0xA4, p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=4)),
        (0xA6, p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=6)),
    ],
)
def test_noise_mode_roundtrip(value, mode):
    assert p.NoiseMode.decode(value) == mode
    assert mode.encode() == value


@pytest.mark.parametrize(
    "mode",
    [
        p.NoiseMode(p.NoiseKind.ANC, "noisy", 4),
        p.NoiseMode(p.NoiseKind.ANC, "wind", 2),
        p.NoiseMode(p.NoiseKind.ANC, "space", 1),
        p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=7),
    ],
)
def test_noise_mode_rejects_invalid(mode):
    with pytest.raises(p.ProtocolError):
        mode.encode()


def test_noise_decode_unknown():
    with pytest.raises(p.ProtocolError):
        p.NoiseMode.decode(0x30)


def test_battery_decode():
    b = p.Battery.decode(bytes([0x63, 0x64 | 0x80, 0x00]))
    assert b.left.percent == 99 and not b.left.charging
    assert b.right.percent == 100 and b.right.charging
    assert b.case.percent is None


def test_version_decode():
    assert p.decode_version(bytes([4, 0, 0, 4, 0, 1])) == ("4.0.0", "4.0.1")


def test_eq_custom_roundtrip():
    raw = bytes.fromhex("ff0302010000000000 0102".replace(" ", ""))
    eq = p.Equalizer.decode(raw)
    assert eq.preset == p.EQ_CUSTOM
    assert eq.gains == (3, 2, 1, 0, 0, 0, 0, 0, 1, 2)
    assert eq.encode() == raw


def test_eq_negative_gains():
    eq = p.Equalizer.custom([-8, -1, 0, 0, 0, 0, 0, 0, 1, 8])
    assert p.Equalizer.decode(eq.encode()).gains == (-8, -1, 0, 0, 0, 0, 0, 0, 1, 8)


def test_eq_preset_uses_table_gains():
    # O firmware devolve ganhos zerados para presets internos.
    eq = p.Equalizer.decode(bytes([1]) + bytes(10))
    assert eq.label() == "Pop"
    assert eq.gains == p.EQ_PRESETS[1][1]
    assert p.Equalizer(3, ()).encode() == bytes([3]) + bytes(10)


def test_eq_rejects_out_of_range():
    with pytest.raises(p.ProtocolError):
        p.Equalizer.custom([9] + [0] * 9).encode()
    with pytest.raises(p.ProtocolError):
        p.Equalizer.custom([0] * 9).encode()


def test_keys_decode_real_value():
    raw = bytes.fromhex("0101020103020403050706 0b07000800000000 00".replace(" ", ""))
    keys = p.decode_keys(raw)
    assert keys[p.Gesture.TAP_LEFT] == 1
    assert keys[p.Gesture.DOUBLE_RIGHT] == 3
    assert keys[p.Gesture.TRIPLE_LEFT] == 7
    assert keys[p.Gesture.TRIPLE_RIGHT] == 11
    assert p.encode_key(p.Gesture.TRIPLE_RIGHT, 11) == b"\x06\x0b"
    with pytest.raises(p.ProtocolError):
        p.encode_key(1, 9)


def test_rename():
    frame = p.cmd_rename("Meu HT05")
    assert frame == bytes([0xFF, 10, 0x18, 8]) + b"Meu HT05"
    assert p.cmd_rename("Fone ção")[3] == len("Fone ção".encode())
    with pytest.raises(p.ProtocolError):
        p.cmd_rename("   ")
    with pytest.raises(p.ProtocolError):
        p.cmd_rename("x" * 33)


def test_decode_name_padded():
    tlvs = dict(p.parse_frame(NAME_FRAME))
    assert p.decode_name(tlvs[p.Cmd.PAIR_NAME]) == "Pedrot "


def test_advertisement():
    ad = p.parse_advertisement(ADV_PAYLOAD)
    assert ad.vendor_id == p.VENDOR_ID_HT05
    assert ad.classic_mac == "84:AC:60:11:22:33"
    assert ad.battery.left.percent == 100
    assert p.parse_advertisement(b"\x00" * 5) is None
