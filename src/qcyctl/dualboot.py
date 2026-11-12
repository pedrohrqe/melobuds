"""Sincroniza a chave de pareamento Bluetooth clássico do Windows para o BlueZ (Linux).

No dual-boot, Windows e Linux usam o mesmo adaptador. Cada pareamento gera uma chave nova,
por isso o fone deixa de conectar no sistema que pareou antes. Este módulo copia a chave
que o Windows guardou no registro para o arquivo `info` do BlueZ.

Procedimento:
1. Parear o fone no Linux (uma vez, para criar o diretório do dispositivo no BlueZ).
2. Iniciar o Windows e parear o fone lá.
3. Voltar ao Linux e executar `sudo qcyctl dualboot`.

Só o registro do Windows é lido. Nada é escrito na partição do Windows.
"""

from __future__ import annotations

import configparser
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

BLUEZ_DIR = Path("/var/lib/bluetooth")
HIVE_REL = Path("Windows/System32/config/SYSTEM")
KEYS_PATH = "Services\\BTHPORT\\Parameters\\Keys"


class DualbootError(RuntimeError):
    pass


def _mac_hex(mac: str) -> str:
    return mac.replace(":", "").lower()


def _mac_colon(hexstr: str) -> str:
    return ":".join(hexstr[i : i + 2] for i in range(0, 12, 2)).upper()


def find_windows_root(explicit: str | None = None) -> Path:
    """Acha uma partição Windows montada (procura o hive SYSTEM)."""
    if explicit:
        root = Path(explicit)
        if not (root / HIVE_REL).is_file():
            raise DualbootError(f"hive SYSTEM não encontrado em {root / HIVE_REL}")
        return root
    with open("/proc/mounts", encoding="utf-8") as fh:
        mounts = [line.split()[1].replace("\\040", " ") for line in fh]
    for mnt in mounts:
        try:
            if (Path(mnt) / HIVE_REL).is_file():
                return Path(mnt)
        except OSError:
            continue  # pontos de montagem sem permissão (/sys, /proc...)
    raise DualbootError(
        "nenhuma partição Windows montada. Monte em modo somente leitura, por exemplo:\n"
        "  udisksctl mount -b /dev/nvme0n1p3 -o ro\n"
        "e execute de novo (ou use --win <ponto de montagem>)."
    )


def read_windows_keys(windows_root: Path) -> dict[str, dict[str, bytes]]:
    """Retorna {adaptador: {dispositivo: chave de 16 bytes}} com MACs em formato AA:BB:..."""
    try:
        from Registry import Registry  # python-registry (extra "dualboot")
    except ImportError as exc:
        raise DualbootError("instale o extra: pip install 'qcyctl[dualboot]'") from exc

    reg = Registry.Registry(str(windows_root / HIVE_REL))
    try:
        current = reg.open("Select").value("Current").value()
    except Registry.RegistryKeyNotFoundException:
        current = 1
    try:
        keys = reg.open(f"ControlSet{current:03d}\\{KEYS_PATH}")
    except Registry.RegistryKeyNotFoundException as exc:
        raise DualbootError("o Windows não tem chaves Bluetooth registradas") from exc

    out: dict[str, dict[str, bytes]] = {}
    for adapter in keys.subkeys():
        devices: dict[str, bytes] = {}
        for value in adapter.values():
            raw = value.value()
            # Chaves de BR/EDR são REG_BINARY de 16 bytes com o MAC como nome.
            if isinstance(raw, bytes) and len(raw) == 16 and len(value.name()) == 12:
                devices[_mac_colon(value.name())] = raw
        if devices:
            out[_mac_colon(adapter.name())] = devices
    return out


@dataclass
class SyncResult:
    adapter: str
    device: str
    old_key: str | None
    new_key: str
    changed: bool


def sync(windows_root: Path, device: str | None = None, dry_run: bool = False) -> list[SyncResult]:
    """Copia as chaves do Windows para o BlueZ. Sem `device`, copia todas que existem nos dois."""
    results: list[SyncResult] = []
    win = read_windows_keys(windows_root)
    for adapter, devices in win.items():
        adapter_dir = BLUEZ_DIR / adapter
        if not adapter_dir.is_dir():
            continue
        for dev_mac, key in devices.items():
            if device and dev_mac != device.upper():
                continue
            info = adapter_dir / dev_mac / "info"
            if not info.is_file():
                continue  # dispositivo nunca pareado no Linux
            cfg = configparser.ConfigParser(interpolation=None)
            cfg.optionxform = str  # o BlueZ diferencia maiúsculas
            cfg.read(info, encoding="utf-8")
            new = key.hex().upper()
            old = cfg.get("LinkKey", "Key", fallback=None)
            changed = old != new
            results.append(SyncResult(adapter, dev_mac, old, new, changed))
            if changed and not dry_run:
                shutil.copy2(info, info.with_name(f"info.bak-{int(time.time())}"))
                if not cfg.has_section("LinkKey"):
                    cfg.add_section("LinkKey")
                    cfg.set("LinkKey", "Type", "4")
                    cfg.set("LinkKey", "PINLength", "0")
                cfg.set("LinkKey", "Key", new)
                with open(info, "w", encoding="utf-8") as fh:
                    cfg.write(fh, space_around_delimiters=False)
    return results


def restart_bluetooth() -> None:
    subprocess.run(["systemctl", "restart", "bluetooth"], check=True)


def run(win: str | None, device: str | None, dry_run: bool) -> int:
    if os.name != "posix":
        raise DualbootError("este comando roda só no Linux")
    root = find_windows_root(win)
    if os.geteuid() != 0:
        raise DualbootError("execute com sudo (o BlueZ guarda as chaves em /var/lib/bluetooth)")
    results = sync(root, device, dry_run)
    if not results:
        print("Nenhum dispositivo pareado nos dois sistemas.")
        return 1
    for r in results:
        state = "atualizada" if r.changed and not dry_run else ("diferente" if r.changed else "igual")
        print(f"{r.device} (adaptador {r.adapter}): chave {state}")
    if any(r.changed for r in results) and not dry_run:
        restart_bluetooth()
        print("Serviço bluetooth reiniciado. Conecte o fone normalmente.")
    return 0
