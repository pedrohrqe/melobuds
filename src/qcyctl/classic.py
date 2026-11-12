"""Conexão de áudio (Bluetooth clássico) do fone pelo BlueZ, via D-Bus. Só Linux.

O controle usa BLE e não exige pareamento. O áudio usa o endereço clássico, que precisa
estar pareado. Este módulo pareia (se preciso), marca como confiável e conecta.
No Windows o sistema reconecta o áudio sozinho; as funções retornam sem fazer nada.
"""

from __future__ import annotations

import asyncio
import logging
import sys
from dataclasses import dataclass

log = logging.getLogger(__name__)

BLUEZ = "org.bluez"
DEVICE_IFACE = "org.bluez.Device1"
PROPS_IFACE = "org.freedesktop.DBus.Properties"
SUPPORTED = sys.platform.startswith("linux")


@dataclass
class ClassicStatus:
    known: bool
    paired: bool = False
    connected: bool = False


def _path(mac: str, adapter: str = "hci0") -> str:
    return f"/org/bluez/{adapter}/dev_{mac.upper().replace(':', '_')}"


async def _device(bus, mac: str):
    from dbus_fast import DBusError

    path = _path(mac)
    try:
        intro = await bus.introspect(BLUEZ, path)
    except DBusError:
        return None  # o BlueZ não conhece este MAC
    obj = bus.get_proxy_object(BLUEZ, path, intro)
    if not any(i.name == DEVICE_IFACE for i in intro.interfaces):
        return None
    return obj.get_interface(DEVICE_IFACE), obj.get_interface(PROPS_IFACE)


async def status(mac: str) -> ClassicStatus:
    if not SUPPORTED:
        return ClassicStatus(known=False)
    from dbus_fast import BusType
    from dbus_fast.aio import MessageBus

    bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    try:
        dev = await _device(bus, mac)
        if dev is None:
            return ClassicStatus(known=False)
        d, _ = dev
        return ClassicStatus(True, await d.get_paired(), await d.get_connected())
    finally:
        bus.disconnect()


async def disconnect(mac: str) -> bool:
    """Desconecta um dispositivo, se o BlueZ o mostra conectado. Retorna True se desconectou.

    No endereço de áudio do HT05, um Connect() nosso logo depois falha (ver `reconnect`).
    Por isso o uso no áudio é só com os fones na caixa: a conexão seguinte parte do fone.
    """
    if not SUPPORTED:
        return False
    from dbus_fast import BusType
    from dbus_fast.aio import MessageBus

    bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    try:
        dev = await _device(bus, mac)
        if dev is None or not await dev[0].get_connected():
            return False
        await dev[0].call_disconnect()
        return True
    finally:
        bus.disconnect()


ADAPTER_PATH = "/org/bluez/hci0"
ADAPTER_IFACE = "org.bluez.Adapter1"


async def adapter_powered() -> bool | None:
    """Estado do Bluetooth do sistema. None = desconhecido (fora do Linux, sem D-Bus)."""
    if not SUPPORTED:
        return None
    from dbus_fast import BusType, DBusError
    from dbus_fast.aio import MessageBus

    try:
        bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    except Exception:  # noqa: BLE001 - sem barramento de sistema
        return None
    try:
        intro = await bus.introspect(BLUEZ, ADAPTER_PATH)
        props = bus.get_proxy_object(BLUEZ, ADAPTER_PATH, intro).get_interface(PROPS_IFACE)
        return bool((await props.call_get(ADAPTER_IFACE, "Powered")).value)
    except DBusError:
        return False  # sem adaptador, ou o serviço Bluetooth parado
    finally:
        bus.disconnect()


async def set_powered(on: bool) -> None:
    """Liga ou desliga o Bluetooth do sistema. Lança DBusError se o sistema recusa."""
    from dbus_fast import BusType, Variant
    from dbus_fast.aio import MessageBus

    bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    try:
        intro = await bus.introspect(BLUEZ, ADAPTER_PATH)
        props = bus.get_proxy_object(BLUEZ, ADAPTER_PATH, intro).get_interface(PROPS_IFACE)
        await props.call_set(ADAPTER_IFACE, "Powered", Variant("b", on))
    finally:
        bus.disconnect()


A2DP_SINK = "0000110b-0000-1000-8000-00805f9b34fb"


async def _connect_audio(d) -> None:
    """Conecta primeiro o perfil de música (A2DP) e depois os outros perfis.

    Um Connect() direto abre o perfil de chamadas (HFP) antes: o sistema escolhe o modo
    "headset", de baixa qualidade, e só troca para o modo normal segundos depois.
    """
    from dbus_fast import DBusError

    try:
        await d.call_connect_profile(A2DP_SINK)
    except DBusError as exc:
        log.info("A2DP primeiro falhou (%s); conexão padrão", exc)
        await d.call_connect()
        return
    try:
        await d.call_connect()  # perfis restantes (chamadas, controle de mídia)
    except DBusError as exc:
        log.info("perfis restantes: %s", exc)


async def ensure_connected(mac: str) -> str:
    """Pareia (se preciso), confia e conecta o áudio. Retorna uma descrição do resultado."""
    if not SUPPORTED:
        return "não suportado"
    from dbus_fast import BusType, DBusError, Variant
    from dbus_fast.aio import MessageBus

    bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    try:
        dev = await _device(bus, mac)
        if dev is None:
            return "o BlueZ não conhece o fone. Coloque o fone em modo de pareamento."
        d, props = dev
        if await d.get_connected():
            return "conectado"
        try:
            if not await d.get_paired():
                log.info("pareando %s", mac)
                await d.call_pair()
            await props.call_set(DEVICE_IFACE, "Trusted", Variant("b", True))
            await _connect_audio(d)
        except DBusError as exc:
            log.info("falha no áudio %s: %s", mac, exc)
            # A falha pode ser uma colisão com a conexão que o próprio fone iniciou.
            # Espera essa conexão terminar antes de declarar o erro.
            for _ in range(3):
                await asyncio.sleep(1)
                if await d.get_connected():
                    return "conectado"
            if "AuthenticationFailed" in exc.type or "AuthenticationRejected" in exc.type:
                return "o fone recusou o pareamento. Coloque o fone em modo de pareamento."
            return f"falha ao conectar o áudio: {exc.text or exc.type}"
        return "conectado"
    finally:
        bus.disconnect()


async def reconnect(mac: str) -> bool:
    """Reinicia o adaptador e reconecta o áudio. O HT05 volta a anunciar BLE por cerca de 60 s.

    Um Device1.Disconnect() seguido de Connect() não serve: com o HT05, o BlueZ fica preso em
    "operation already in progress" e recusa toda conexão até o adaptador reiniciar.
    Por isso o ciclo do adaptador faz a desconexão.
    """
    if not SUPPORTED:
        return False
    from dbus_fast import BusType, DBusError, Variant
    from dbus_fast.aio import MessageBus

    bus = await MessageBus(bus_type=BusType.SYSTEM).connect()
    try:
        adapter = "/org/bluez/hci0"
        intro = await bus.introspect(BLUEZ, adapter)
        props = bus.get_proxy_object(BLUEZ, adapter, intro).get_interface(PROPS_IFACE)
        try:
            await props.call_set("org.bluez.Adapter1", "Powered", Variant("b", False))
            await asyncio.sleep(2)
        finally:
            await props.call_set("org.bluez.Adapter1", "Powered", Variant("b", True))
        await asyncio.sleep(3)
        for tentativa in range(4):
            dev = await _device(bus, mac)
            if dev is None:
                return False
            d, _ = dev
            try:
                if not await d.get_connected():
                    await _connect_audio(d)
                return True
            except DBusError as exc:
                log.info("falha ao reconectar %s (tentativa %d): %s", mac, tentativa + 1, exc)
                await asyncio.sleep(3)
        return False
    finally:
        bus.disconnect()
