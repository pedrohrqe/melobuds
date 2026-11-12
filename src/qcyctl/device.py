"""Conexão BLE com o QCY HT05 e estado do dispositivo."""

from __future__ import annotations

import asyncio
import logging
import time
from collections.abc import Callable
from dataclasses import dataclass, field

from bleak import BleakClient, BleakScanner
from bleak.backends.characteristic import BleakGATTCharacteristic
from bleak.backends.device import BLEDevice

from . import protocol as p

log = logging.getLogger(__name__)


@dataclass
class DeviceState:
    connected: bool = False
    name: str = ""
    battery: p.Battery | None = None
    version: tuple[str, str] | None = None
    noise: p.NoiseMode | None = None
    game_mode: bool | None = None
    sleep_mode: bool | None = None
    balance: int | None = None
    pair_name: str | None = None
    eq: p.Equalizer | None = None
    keys: dict[int, int] = field(default_factory=dict)


@dataclass
class FoundDevice:
    address: str
    name: str
    advertisement: p.Advertisement


async def discover(timeout: float = 8.0) -> list[FoundDevice]:
    """Procura fones QCY pelo company ID do manufacturer data."""
    found = await BleakScanner.discover(timeout=timeout, return_adv=True)
    out: list[FoundDevice] = []
    for addr, (dev, adv) in found.items():
        for cid in (p.COMPANY_ID, p.COMPANY_ID_ALT):
            payload = adv.manufacturer_data.get(cid)
            if payload and (parsed := p.parse_advertisement(payload)):
                out.append(FoundDevice(addr, dev.name or adv.local_name or "QCY", parsed))
    return out


def _is_qcy(adv) -> bool:
    return any(cid in adv.manufacturer_data for cid in (p.COMPANY_ID, p.COMPANY_ID_ALT))


async def find_first(timeout: float = 10.0) -> BLEDevice | None:
    """Retorna o primeiro fone QCY que anunciar, sem esperar o fim do scan."""
    found = await find_first_with_adv(timeout)
    return found[0] if found else None


async def find_first_with_adv(timeout: float = 10.0) -> tuple[BLEDevice, p.Advertisement] | None:
    """Como find_first, mas também retorna o anúncio (com o MAC clássico do áudio)."""
    seen: dict[str, p.Advertisement] = {}

    def match(dev: BLEDevice, adv) -> bool:
        for cid in (p.COMPANY_ID, p.COMPANY_ID_ALT):
            payload = adv.manufacturer_data.get(cid)
            if payload and (parsed := p.parse_advertisement(payload)):
                seen[dev.address] = parsed
                return True
        return False

    dev = await BleakScanner.find_device_by_filter(match, timeout=timeout)
    return (dev, seen[dev.address]) if dev else None


class HT05:
    """Cliente do HT05. As escritas passam por uma fila serial, como no app oficial."""

    def __init__(self, address: str | BLEDevice) -> None:
        self.address = address
        self.state = DeviceState()
        self._client: BleakClient | None = None
        self._chars: dict[str, BleakGATTCharacteristic] = {}
        self._lock = asyncio.Lock()
        self._listeners: list[Callable[[DeviceState], None]] = []
        self.last_rx = 0.0   # instante (time.monotonic) da última notificação do fone

    # --- eventos ---

    def on_change(self, callback: Callable[[DeviceState], None]) -> None:
        self._listeners.append(callback)

    def _emit(self) -> None:
        for cb in self._listeners:
            try:
                cb(self.state)
            except Exception:  # um listener com erro não derruba os outros
                log.exception("erro em listener")

    # --- conexão ---

    @property
    def is_connected(self) -> bool:
        return bool(self._client and self._client.is_connected)

    async def connect(self, timeout: float = 15.0) -> None:
        self._client = BleakClient(self.address, timeout=timeout,
                                   disconnected_callback=self._on_disconnect)
        try:
            await self._client.connect()
            service = self._client.services.get_service(p.SERVICE)
            if service is None:
                raise p.ProtocolError("serviço de controle QCY (A001) não encontrado")
            # O HT05 repete UUIDs em outro serviço, por isso a busca é dentro do A001.
            self._chars = {c.uuid: c for c in service.characteristics}
            for uuid in (p.CHAR_RESP, p.CHAR_BATTERY):
                if uuid in self._chars:
                    await self._client.start_notify(self._chars[uuid], self._on_notify)
            self.state.connected = True
            self.state.name = getattr(self._client, "name", "") or "QCY HT05"
            await self.refresh()
        except BaseException:
            # Sem esta limpeza, o BlueZ mantém o fone como "conectado" e toda tentativa
            # seguinte falha com "Not connected".
            self.state.connected = False
            try:
                await asyncio.wait_for(self._client.disconnect(), 5)
            except Exception:
                log.debug("falha na limpeza da conexão", exc_info=True)
            raise

    async def disconnect(self) -> None:
        if self._client:
            await self._client.disconnect()

    def _on_disconnect(self, _client: BleakClient) -> None:
        self.state.connected = False
        self._emit()

    # --- I/O de baixo nível ---

    async def _write(self, uuid: str, data: bytes) -> None:
        if not self.is_connected:
            raise ConnectionError("fone desconectado")
        async with self._lock:
            log.debug("-> %s %s", uuid[4:8], data.hex(" "))
            await self._client.write_gatt_char(self._chars[uuid], data, response=False)
            await asyncio.sleep(0.05)

    async def _read(self, uuid: str) -> bytes:
        if not self.is_connected:
            raise ConnectionError("fone desconectado")
        async with self._lock:
            return bytes(await self._client.read_gatt_char(self._chars[uuid]))

    async def send_command(self, frame: bytes) -> None:
        await self._write(p.CHAR_CMD, frame)

    def _on_notify(self, char: BleakGATTCharacteristic, data: bytearray) -> None:
        data = bytes(data)
        self.last_rx = time.monotonic()
        log.debug("<- %s %s", char.uuid[4:8], data.hex(" "))
        try:
            if char.uuid == p.CHAR_BATTERY:
                self.state.battery = p.Battery.decode(data)
            elif char.uuid == p.CHAR_RESP:
                self._apply_tlvs(p.parse_frame(data))
        except p.ProtocolError as exc:
            log.warning("%s", exc)
            return
        self._emit()

    def _apply_tlvs(self, tlvs: list[tuple[int, bytes]]) -> None:
        for cmd_id, payload in tlvs:
            if not payload:
                continue
            if cmd_id == p.Cmd.PAIR_NAME:
                self.state.pair_name = p.decode_name(payload)
                continue
            value = payload[0]
            if cmd_id == p.Cmd.NOISE_MODE:
                try:
                    self.state.noise = p.NoiseMode.decode(value)
                except p.ProtocolError as exc:
                    log.warning("%s", exc)
            elif cmd_id == p.Cmd.GAME_MODE:
                self.state.game_mode = value == p.ON
            elif cmd_id == p.Cmd.SLEEP_MODE:
                self.state.sleep_mode = value == p.ON
            elif cmd_id == p.Cmd.BALANCE:
                self.state.balance = value

    # --- leitura de estado ---

    async def refresh(self) -> None:
        """Lê todo o estado do fone."""
        self._apply_tlvs(p.parse_frame(await self._read(p.CHAR_RESP)))
        self.state.battery = p.Battery.decode(await self._read(p.CHAR_BATTERY))
        self.state.version = p.decode_version(await self._read(p.CHAR_VERSION))
        self.state.eq = p.Equalizer.decode(await self._read(p.CHAR_EQ))
        self.state.keys = p.decode_keys(await self._read(p.CHAR_KEYS))
        # A resposta do nome chega por notificação.
        await self.send_command(p.cmd_request(p.Cmd.PAIR_NAME))
        await asyncio.sleep(0.4)
        self._emit()

    # --- comandos ---

    async def rename(self, name: str) -> None:
        await self.send_command(p.cmd_rename(name))
        await asyncio.sleep(0.3)
        await self.send_command(p.cmd_request(p.Cmd.PAIR_NAME))

    async def set_noise(self, mode: p.NoiseMode) -> None:
        await self.send_command(p.cmd_noise(mode))

    async def set_game_mode(self, on: bool) -> None:
        await self.send_command(p.cmd_set(p.Cmd.GAME_MODE, p.ON if on else p.OFF))

    async def set_sleep_mode(self, on: bool) -> None:
        await self.send_command(p.cmd_set(p.Cmd.SLEEP_MODE, p.ON if on else p.OFF))

    async def set_eq(self, eq: p.Equalizer) -> None:
        await self._write(p.CHAR_EQ, eq.encode())
        self.state.eq = p.Equalizer.decode(await self._read(p.CHAR_EQ))
        self._emit()

    async def set_key(self, gesture: int, action: int) -> None:
        await self._write(p.CHAR_KEYS, p.encode_key(gesture, action))
        self.state.keys = p.decode_keys(await self._read(p.CHAR_KEYS))
        self._emit()
