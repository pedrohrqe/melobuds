"""Codificação e decodificação do protocolo BLE do QCY HT05.

Funções puras, sem I/O. Ver docs/protocol.md.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum

# --- GATT -------------------------------------------------------------------

def _uuid(short: int) -> str:
    return f"0000{short:04x}-0000-1000-8000-00805f9b34fb"


SERVICE = _uuid(0xA001)
CHAR_VERSION = _uuid(0x0007)   # read: versão L (3 bytes) + R (3 bytes)
CHAR_BATTERY = _uuid(0x0008)   # read/notify: [L, R, caixa]
CHAR_EQ = _uuid(0x000B)        # read/write: [tipo, 10 ganhos em dB]
CHAR_KEYS = _uuid(0x000D)      # read/write: pares [evento, função]
CHAR_CMD = _uuid(0x1001)       # write: frames TLV
CHAR_RESP = _uuid(0x1002)      # read/notify: frames TLV

COMPANY_ID = 0x521C            # manufacturer data no anúncio BLE
COMPANY_ID_ALT = 0x522C
VENDOR_ID_HT05 = 0x4D41

# --- IDs de comando TLV -----------------------------------------------------


class Cmd(IntEnum):
    RESET_DEFAULT = 0x01
    FACTORY_RESET = 0x03
    EAR_DETECT = 0x06
    VOLUME = 0x08
    GAME_MODE = 0x09
    NOISE_MODE = 0x0C
    SLEEP_MODE = 0x10
    BALANCE = 0x16
    PAIR_NAME = 0x18
    REQUEST = 0xFE


FRAME_HEADER = 0xFF
ON, OFF = 0x01, 0x02  # booleanos do protocolo


class ProtocolError(ValueError):
    pass


def build_frame(tlvs: list[tuple[int, bytes]]) -> bytes:
    """Monta um frame `FF len [id len payload]...`."""
    body = bytearray()
    for cmd_id, payload in tlvs:
        if len(payload) > 255:
            raise ProtocolError("payload grande demais")
        body += bytes([cmd_id, len(payload)]) + payload
    if len(body) > 255:
        raise ProtocolError("frame grande demais")
    return bytes([FRAME_HEADER, len(body)]) + bytes(body)


def parse_frame(data: bytes) -> list[tuple[int, bytes]]:
    """Separa um frame em uma lista de (id, payload)."""
    if len(data) < 2 or data[0] != FRAME_HEADER or data[1] != len(data) - 2:
        raise ProtocolError(f"frame inválido: {data.hex(' ')}")
    out: list[tuple[int, bytes]] = []
    i = 2
    while i + 2 <= len(data):
        cmd_id, length = data[i], data[i + 1]
        payload = data[i + 2 : i + 2 + length]
        if len(payload) != length:
            raise ProtocolError(f"TLV truncado: {data.hex(' ')}")
        out.append((cmd_id, bytes(payload)))
        i += 2 + length
    return out


def cmd_set(cmd_id: int, value: int) -> bytes:
    return build_frame([(cmd_id, bytes([value & 0xFF]))])


def cmd_request(*cmd_ids: int) -> bytes:
    """Pede ao fone o valor atual de um ou mais IDs."""
    return build_frame([(Cmd.REQUEST, bytes([c])) for c in cmd_ids])


# --- Modo de ruído ----------------------------------------------------------
# Valores do painel oficial do HT05 (modelId 19777, firmware >= 2.0.9).


class NoiseKind(IntEnum):
    NORMAL = 0
    ANC = 1
    TRANSPARENCY = 2


ANC_SCENES: dict[str, tuple[str, int]] = {
    # nome interno: (rótulo, valor base do nível 1)
    "indoor": ("Interno", 0x41),
    "commute": ("Transporte", 0x51),
    "noisy": ("Ruidoso", 0x61),
    "wind": ("Anti-vento", 0x71),
}
ANC_LEVELS = 3            # níveis 1..3 (exceto "wind", que tem 1)
NOISE_NORMAL = 0x02
TRANSPARENCY_VOICE = 0xA0  # transparência com realce de voz
TRANSPARENCY_MIN, TRANSPARENCY_MAX = 0xA1, 0xA6
TRANSPARENCY_DEFAULT = 0xA4
ANC_DEFAULT = 0x63


@dataclass(frozen=True)
class NoiseMode:
    kind: NoiseKind
    scene: str | None = None   # só para ANC
    level: int = 0             # ANC: 1..3; transparência: 1..6, 0 = voz

    def encode(self) -> int:
        if self.kind is NoiseKind.NORMAL:
            return NOISE_NORMAL
        if self.kind is NoiseKind.TRANSPARENCY:
            if self.level == 0:
                return TRANSPARENCY_VOICE
            if not 1 <= self.level <= 6:
                raise ProtocolError("nível de transparência deve ser 0..6")
            return TRANSPARENCY_MIN + self.level - 1
        if self.scene not in ANC_SCENES:
            raise ProtocolError(f"cena ANC desconhecida: {self.scene}")
        max_level = 1 if self.scene == "wind" else ANC_LEVELS
        if not 1 <= self.level <= max_level:
            raise ProtocolError(f"nível ANC deve ser 1..{max_level}")
        return ANC_SCENES[self.scene][1] + self.level - 1

    @classmethod
    def decode(cls, value: int) -> NoiseMode:
        if value == NOISE_NORMAL:
            return cls(NoiseKind.NORMAL)
        if value == TRANSPARENCY_VOICE:
            return cls(NoiseKind.TRANSPARENCY, level=0)
        if TRANSPARENCY_MIN <= value <= TRANSPARENCY_MAX:
            return cls(NoiseKind.TRANSPARENCY, level=value - TRANSPARENCY_MIN + 1)
        for name, (_, base) in ANC_SCENES.items():
            if base <= value < base + ANC_LEVELS:
                return cls(NoiseKind.ANC, scene=name, level=value - base + 1)
        raise ProtocolError(f"modo de ruído desconhecido: 0x{value:02x}")

    def label(self) -> str:
        if self.kind is NoiseKind.NORMAL:
            return "Normal"
        if self.kind is NoiseKind.TRANSPARENCY:
            return "Transparência (voz)" if self.level == 0 else f"Transparência {self.level}"
        return f"ANC {ANC_SCENES[self.scene][0]} {self.level}"


def cmd_noise(mode: NoiseMode) -> bytes:
    return cmd_set(Cmd.NOISE_MODE, mode.encode())


# --- Nome de pareamento (TLV 0x18) -----------------------------------------

PAIR_NAME_MAX_BYTES = 32


def cmd_rename(name: str) -> bytes:
    """Novo nome Bluetooth. O fone aplica o nome na próxima conexão clássica."""
    raw = name.encode("utf-8")
    if not name.strip():
        raise ProtocolError("o nome não pode ficar vazio")
    if len(raw) > PAIR_NAME_MAX_BYTES:
        raise ProtocolError(f"nome com mais de {PAIR_NAME_MAX_BYTES} bytes UTF-8")
    return build_frame([(Cmd.PAIR_NAME, raw)])


def decode_name(payload: bytes) -> str:
    """O fone devolve o nome em 32 bytes completados com zeros."""
    return payload.split(b"\0", 1)[0].decode("utf-8", "replace")


# --- Bateria e versão -------------------------------------------------------


@dataclass(frozen=True)
class BatteryLevel:
    percent: int | None   # None = desconhecido (fora da caixa, desligado)
    charging: bool

    @classmethod
    def decode(cls, byte: int) -> BatteryLevel:
        pct = byte & 0x7F
        return cls(pct if 1 <= pct <= 100 else None, bool(byte & 0x80))

    def label(self) -> str:
        if self.percent is None:
            return "—"
        return f"{self.percent}%" + (" ⚡" if self.charging else "")


@dataclass(frozen=True)
class Battery:
    left: BatteryLevel
    right: BatteryLevel
    case: BatteryLevel

    @classmethod
    def decode(cls, data: bytes) -> Battery:
        if len(data) < 3:
            raise ProtocolError(f"bateria inválida: {data.hex(' ')}")
        return cls(*(BatteryLevel.decode(b) for b in data[:3]))


def decode_version(data: bytes) -> tuple[str, str]:
    """Retorna (versão esquerda, versão direita)."""
    left = ".".join(str(b) for b in data[0:3])
    right = ".".join(str(b) for b in data[3:6]) if len(data) >= 6 else left
    return left, right


# --- Equalizador (characteristic 000B) -------------------------------------

EQ_BANDS = ("31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k")
EQ_MIN_DB, EQ_MAX_DB = -8, 8
EQ_CUSTOM = 0xFF

# Presets internos do firmware: o índice é o tipo; os ganhos são só para exibição.
EQ_PRESETS: list[tuple[str, tuple[int, ...]]] = [
    ("Padrão", (0, 0, 0, 0, 0, 0, 0, 0, 0, 0)),
    ("Pop", (-6, 5, -3, -2, 5, 4, -4, -3, 6, 4)),
    ("Graves", (6, 5, 4, 3, 0, 0, 0, 0, 0, 0)),
    ("Rock", (5, 4, 2, 1, -3, -4, 2, 1, 4, 5)),
    ("Suave", (5, -5, -4, -4, 3, 2, 4, 4, 0, 0)),
    ("Clássico", (5, 5, 1, 2, -1, 1, -4, -6, -7, -8)),
]


@dataclass(frozen=True)
class Equalizer:
    preset: int                  # 0..5 ou EQ_CUSTOM
    gains: tuple[int, ...]       # 10 valores em dB

    @classmethod
    def decode(cls, data: bytes) -> Equalizer:
        if len(data) < 1 + len(EQ_BANDS):
            raise ProtocolError(f"EQ inválido: {data.hex(' ')}")
        preset = data[0]
        gains = tuple(int.from_bytes(bytes([b]), "big", signed=True) for b in data[1:11])
        if preset < len(EQ_PRESETS):
            # O firmware devolve ganhos zerados para presets internos.
            gains = EQ_PRESETS[preset][1]
        return cls(preset, gains)

    def encode(self) -> bytes:
        if self.preset != EQ_CUSTOM:
            if not 0 <= self.preset < len(EQ_PRESETS):
                raise ProtocolError("preset de EQ inválido")
            return bytes([self.preset]) + bytes(len(EQ_BANDS))
        if len(self.gains) != len(EQ_BANDS):
            raise ProtocolError("EQ precisa de 10 ganhos")
        if any(not EQ_MIN_DB <= g <= EQ_MAX_DB for g in self.gains):
            raise ProtocolError(f"ganho fora de {EQ_MIN_DB}..{EQ_MAX_DB} dB")
        return bytes([EQ_CUSTOM]) + bytes(g & 0xFF for g in self.gains)

    @classmethod
    def custom(cls, gains: list[int] | tuple[int, ...]) -> Equalizer:
        return cls(EQ_CUSTOM, tuple(gains))

    def label(self) -> str:
        return "Personalizado" if self.preset == EQ_CUSTOM else EQ_PRESETS[self.preset][0]


# --- Controles de toque (characteristic 000D) -------------------------------


class Gesture(IntEnum):
    TAP_LEFT = 1
    TAP_RIGHT = 2
    DOUBLE_LEFT = 3
    DOUBLE_RIGHT = 4
    TRIPLE_LEFT = 5
    TRIPLE_RIGHT = 6


GESTURE_LABELS = {
    Gesture.TAP_LEFT: ("Toque", "Esquerdo"),
    Gesture.TAP_RIGHT: ("Toque", "Direito"),
    Gesture.DOUBLE_LEFT: ("Toque duplo", "Esquerdo"),
    Gesture.DOUBLE_RIGHT: ("Toque duplo", "Direito"),
    Gesture.TRIPLE_LEFT: ("Toque triplo", "Esquerdo"),
    Gesture.TRIPLE_RIGHT: ("Toque triplo", "Direito"),
}

KEY_ACTIONS: dict[int, str] = {
    0: "Nenhuma",
    1: "Play/Pausa",
    2: "Faixa anterior",
    3: "Próxima faixa",
    4: "Assistente de voz",
    5: "Volume +",
    6: "Volume −",
    7: "Modo jogo",
    11: "Alternar ANC",
}


def decode_keys(data: bytes) -> dict[int, int]:
    """Retorna {evento: função}. Ignora pares vazios (evento 0)."""
    out: dict[int, int] = {}
    for i in range(0, len(data) - 1, 2):
        event, func = data[i], data[i + 1]
        if event:
            out[event] = func
    return out


def encode_key(gesture: int, action: int) -> bytes:
    if action not in KEY_ACTIONS:
        raise ProtocolError(f"ação de toque desconhecida: {action}")
    return bytes([int(gesture), action])


# --- Anúncio BLE ------------------------------------------------------------


@dataclass(frozen=True)
class Advertisement:
    vendor_id: int
    battery: Battery
    classic_mac: str


def parse_advertisement(payload: bytes) -> Advertisement | None:
    """Lê o manufacturer data (sem o company ID) do anúncio do fone."""
    if len(payload) < 17:
        return None
    vendor = (payload[0] << 8) | payload[1]
    battery = Battery.decode(payload[5:8])
    mac = bytes([payload[12], payload[11], payload[13], payload[16], payload[15], payload[14]])
    return Advertisement(vendor, battery, ":".join(f"{b:02X}" for b in mac))
