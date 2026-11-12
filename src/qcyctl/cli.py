"""CLI do qcyctl: descoberta e testes de protocolo do QCY HT05."""

from __future__ import annotations

import argparse
import asyncio
import sys

from bleak import BleakClient, BleakScanner

from . import protocol as p
from .device import HT05, discover, find_first


async def cmd_scan(args: argparse.Namespace) -> None:
    """Lista fones QCY próximos (ou todos os dispositivos BLE com --all)."""
    if not args.all:
        for d in await discover(args.timeout):
            ad = d.advertisement
            b = ad.battery
            print(f"{d.address}  modelo=0x{ad.vendor_id:04x}  clássico={ad.classic_mac}  "
                  f"bateria L={b.left.label()} R={b.right.label()} caixa={b.case.label()}")
        return
    found = await BleakScanner.discover(timeout=args.timeout, return_adv=True)
    for addr, (dev, adv) in sorted(found.items(), key=lambda kv: -kv[1][1].rssi):
        print(f"{addr}  rssi={adv.rssi:4d}  {dev.name or adv.local_name or '?'}")
        for cid, data in adv.manufacturer_data.items():
            print(f"    manufacturer 0x{cid:04x}: {data.hex()}")


async def _open(args: argparse.Namespace) -> HT05:
    address = args.address
    if not address:
        address = await find_first(10.0)
        if address is None:
            raise SystemExit("nenhum fone QCY encontrado. Tire os fones da caixa e tente de novo.")
    dev = HT05(address)
    await dev.connect()
    return dev


def _print_state(s) -> None:
    b = s.battery
    print(f"Nome:       {s.pair_name!r}" if s.pair_name is not None else "Nome:       ?")
    print(f"Firmware:   L {s.version[0]}  R {s.version[1]}" if s.version else "Firmware:   ?")
    if b:
        print(f"Bateria:    L {b.left.label()}  R {b.right.label()}  caixa {b.case.label()}")
    print(f"Ruído:      {s.noise.label() if s.noise else '?'}")
    print(f"Modo jogo:  {'ligado' if s.game_mode else 'desligado'}")
    print(f"Modo sono:  {'ligado' if s.sleep_mode else 'desligado'}")
    if s.eq:
        print(f"EQ:         {s.eq.label()}  {list(s.eq.gains)}")
    for g, (gesture, side) in p.GESTURE_LABELS.items():
        action = s.keys.get(int(g))
        if action is not None:
            print(f"Toque:      {gesture} {side}: {p.KEY_ACTIONS.get(action, hex(action))}")


def _parse_noise(text: str) -> p.NoiseMode:
    parts = text.lower().split()
    kind = parts[0]
    if kind in ("off", "normal"):
        return p.NoiseMode(p.NoiseKind.NORMAL)
    if kind in ("transparency", "trans", "t"):
        return p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=int(parts[1]) if len(parts) > 1 else 4)
    if kind == "anc":
        scene = parts[1] if len(parts) > 1 else "noisy"
        level = int(parts[2]) if len(parts) > 2 else (1 if scene == "wind" else 3)
        return p.NoiseMode(p.NoiseKind.ANC, scene=scene, level=level)
    raise SystemExit(f"modo inválido: {text}")


async def cmd_status(args: argparse.Namespace) -> None:
    dev = await _open(args)
    try:
        _print_state(dev.state)
    finally:
        await dev.disconnect()


async def cmd_noise(args: argparse.Namespace) -> None:
    mode = _parse_noise(" ".join(args.mode))
    dev = await _open(args)
    try:
        await dev.set_noise(mode)
        await asyncio.sleep(0.5)
        print(f"Ruído: {dev.state.noise.label() if dev.state.noise else '?'}")
    finally:
        await dev.disconnect()


async def cmd_eq(args: argparse.Namespace) -> None:
    if args.gains:
        eq = p.Equalizer.custom([int(x) for x in args.gains.split(",")])
    else:
        names = [n.lower() for n, _ in p.EQ_PRESETS]
        if args.preset.lower() not in names:
            raise SystemExit(f"preset inválido. Opções: {', '.join(n for n, _ in p.EQ_PRESETS)}")
        eq = p.Equalizer(names.index(args.preset.lower()), ())
    dev = await _open(args)
    try:
        await dev.set_eq(eq)
        print(f"EQ: {dev.state.eq.label()}  {list(dev.state.eq.gains)}")
    finally:
        await dev.disconnect()


async def cmd_game(args: argparse.Namespace) -> None:
    dev = await _open(args)
    try:
        await dev.set_game_mode(args.state == "on")
        await asyncio.sleep(0.5)
        print(f"Modo jogo: {'ligado' if dev.state.game_mode else 'desligado'}")
    finally:
        await dev.disconnect()


async def cmd_rename(args: argparse.Namespace) -> None:
    p.cmd_rename(args.name)  # valida antes de conectar
    dev = await _open(args)
    try:
        await dev.rename(args.name)
        await asyncio.sleep(1.0)
        print(f"Nome: {dev.state.pair_name!r}")
        print("O novo nome aparece depois que o fone reconectar (guarde na caixa e tire de novo).")
    finally:
        await dev.disconnect()


async def cmd_key(args: argparse.Namespace) -> None:
    dev = await _open(args)
    try:
        await dev.set_key(args.gesture, args.action)
        _print_state(dev.state)
    finally:
        await dev.disconnect()


async def cmd_probe(args: argparse.Namespace) -> None:
    """Conecta e mostra todos os serviços e characteristics GATT."""
    async with BleakClient(args.address, timeout=args.timeout) as client:
        for service in client.services:
            print(f"[service] {service.uuid}  {service.description}")
            for char in service.characteristics:
                props = ",".join(char.properties)
                line = f"   [char] {char.uuid}  handle=0x{char.handle:04x}  ({props})"
                if "read" in char.properties:
                    try:
                        value = await client.read_gatt_char(char)
                        line += f"  = {value.hex()}  {value!r}"
                    except Exception as exc:  # noqa: BLE001 - só diagnóstico
                        line += f"  (erro de leitura: {exc})"
                print(line)


async def cmd_raw(args: argparse.Namespace) -> None:
    """Envia bytes em hex para uma characteristic e mostra as notificações."""

    def on_notify(char, data: bytearray) -> None:
        print(f"<- {char.uuid[4:8]}@0x{char.handle:04x}: {data.hex(' ')}")

    def ref(value: str):
        # Aceita handle ("0x0014") ou UUID. O HT05 repete UUIDs em serviços diferentes.
        return int(value, 16) if value.lower().startswith("0x") else value

    async with BleakClient(args.address, timeout=args.timeout) as client:
        for uuid in args.notify:
            await client.start_notify(ref(uuid), on_notify)
        for payload in args.hex:
            data = bytes.fromhex(payload)
            print(f"-> {args.write}: {data.hex(' ')}")
            await client.write_gatt_char(ref(args.write), data, response=not args.no_response)
            await asyncio.sleep(args.wait)


async def cmd_monitor(args: argparse.Namespace) -> None:
    """Assina todas as characteristics com notify e mostra o que chega."""

    def on_notify(char, data: bytearray) -> None:
        print(f"<- {char.uuid[4:8]}@0x{char.handle:04x}: {data.hex(' ')}", flush=True)

    async with BleakClient(args.address, timeout=args.timeout) as client:
        for service in client.services:
            if args.all_services or service.uuid.startswith("0000a001"):
                for char in service.characteristics:
                    if "notify" in char.properties:
                        await client.start_notify(char, on_notify)
        print(f"monitorando por {args.seconds:.0f} s...", flush=True)
        await asyncio.sleep(args.seconds)


async def cmd_dualboot(args: argparse.Namespace) -> None:
    from .dualboot import DualbootError, run

    try:
        rc = run(args.win, args.device, args.dry_run)
    except DualbootError as exc:
        raise SystemExit(f"erro: {exc}") from exc
    if rc:
        raise SystemExit(rc)


async def cmd_audio(args: argparse.Namespace) -> None:
    from . import classic
    from .device import find_first_with_adv

    mac = args.classic
    if not mac:
        found = await find_first_with_adv(15.0)
        if not found:
            raise SystemExit("nenhum fone QCY encontrado. Tire os fones da caixa e tente de novo.")
        mac = found[1].classic_mac
    print(f"Áudio {mac}: {await classic.ensure_connected(mac)}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="qcyctl")
    sub = parser.add_subparsers(dest="cmd", required=True)

    def add_addr(sp):
        sp.add_argument("--address", "-a", help="endereço BLE (padrão: o primeiro QCY encontrado)")

    sp = sub.add_parser("status", help="mostra bateria, modos, EQ e toques")
    add_addr(sp)
    sp.set_defaults(func=cmd_status)

    sp = sub.add_parser("noise", help="modo de ruído: off | anc [indoor|commute|noisy|wind] [1-3] | transparency [0-6]")
    add_addr(sp)
    sp.add_argument("mode", nargs="+")
    sp.set_defaults(func=cmd_noise)

    sp = sub.add_parser("eq", help="EQ: preset por nome ou --gains com 10 valores (-8..8)")
    add_addr(sp)
    g = sp.add_mutually_exclusive_group(required=True)
    g.add_argument("preset", nargs="?")
    g.add_argument("--gains", help="ex.: 3,2,1,0,0,0,0,0,1,2")
    sp.set_defaults(func=cmd_eq)

    sp = sub.add_parser("game", help="modo jogo (baixa latência)")
    add_addr(sp)
    sp.add_argument("state", choices=["on", "off"])
    sp.set_defaults(func=cmd_game)

    sp = sub.add_parser("rename", help="muda o nome Bluetooth do fone (até 32 bytes)")
    add_addr(sp)
    sp.add_argument("name")
    sp.set_defaults(func=cmd_rename)

    sp = sub.add_parser("audio", help="pareia (se preciso) e conecta o áudio do fone (Linux)")
    sp.add_argument("--classic", help="MAC clássico (padrão: lido do anúncio BLE)")
    sp.set_defaults(func=cmd_audio)

    sp = sub.add_parser("dualboot", help="copia a chave de pareamento do Windows para o Linux (sudo)")
    sp.add_argument("--win", help="ponto de montagem da partição Windows")
    sp.add_argument("--device", help="MAC clássico do fone (padrão: todos)")
    sp.add_argument("--dry-run", action="store_true", help="só compara, não altera")
    sp.set_defaults(func=cmd_dualboot)

    sp = sub.add_parser("key", help="remapeia toque: gesto 1-6, ação (0-7, 11)")
    add_addr(sp)
    sp.add_argument("gesture", type=int, choices=range(1, 7))
    sp.add_argument("action", type=int, choices=sorted(p.KEY_ACTIONS))
    sp.set_defaults(func=cmd_key)

    sp = sub.add_parser("scan", help="lista fones QCY (--all: todos os BLE)")
    sp.add_argument("--timeout", type=float, default=8.0)
    sp.add_argument("--all", action="store_true", help="mostra todos, não só QCY")
    sp.set_defaults(func=cmd_scan)

    sp = sub.add_parser("probe", help="lista serviços GATT de um dispositivo")
    sp.add_argument("address")
    sp.add_argument("--timeout", type=float, default=15.0)
    sp.set_defaults(func=cmd_probe)

    sp = sub.add_parser("raw", help="envia bytes crus e mostra notificações")
    sp.add_argument("address")
    sp.add_argument("--write", required=True, help="UUID da characteristic de escrita")
    sp.add_argument("--notify", action="append", default=[], help="UUID para notificações")
    sp.add_argument("--no-response", action="store_true")
    sp.add_argument("--wait", type=float, default=1.5)
    sp.add_argument("--timeout", type=float, default=15.0)
    sp.add_argument("hex", nargs="+", help="pacotes em hex, ex.: aa0102")
    sp.set_defaults(func=cmd_raw)

    sp = sub.add_parser("monitor", help="mostra notificações por um tempo")
    sp.add_argument("address")
    sp.add_argument("--seconds", type=float, default=60.0)
    sp.add_argument("--all-services", action="store_true")
    sp.add_argument("--timeout", type=float, default=15.0)
    sp.set_defaults(func=cmd_monitor)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        asyncio.run(args.func(args))
    except KeyboardInterrupt:
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
