"""Gera um .deb sem dpkg-deb: um arquivo `ar` com debian-binary, control.tar.gz e data.tar.gz.

Uso: python packaging/linux/make_deb.py <versão> <saída.deb>   (na raiz do projeto)
"""

from __future__ import annotations

import io
import sys
import tarfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def _tar(files: list[tuple[str, Path | bytes, int]], symlinks: list[tuple[str, str]] = ()) -> bytes:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz", format=tarfile.GNU_FORMAT) as tar:
        dirs: set[str] = set()
        def add_dirs(path: str) -> None:
            parts = path.strip("./").split("/")[:-1]
            for i in range(1, len(parts) + 1):
                d = "./" + "/".join(parts[:i])
                if d not in dirs:
                    dirs.add(d)
                    info = tarfile.TarInfo(d)
                    info.type, info.mode, info.mtime = tarfile.DIRTYPE, 0o755, int(time.time())
                    tar.addfile(info)
        for name, src, mode in files:
            add_dirs(name)
            data = src if isinstance(src, bytes) else src.read_bytes()
            info = tarfile.TarInfo(name)
            info.size, info.mode, info.mtime = len(data), mode, int(time.time())
            tar.addfile(info, io.BytesIO(data))
        for name, target in symlinks:
            add_dirs(name)
            info = tarfile.TarInfo(name)
            info.type, info.linkname, info.mode = tarfile.SYMTYPE, target, 0o777
            tar.addfile(info)
    return buf.getvalue()


def _ar(members: list[tuple[str, bytes]]) -> bytes:
    out = bytearray(b"!<arch>\n")
    for name, data in members:
        header = f"{name:<16}{int(time.time()):<12}0     0     100644  {len(data):<10}`\n"
        out += header.encode() + data + (b"\n" if len(data) % 2 else b"")
    return bytes(out)


def main(version: str, target: str) -> None:
    dist = ROOT / "dist" / "qcyctl"
    files: list[tuple[str, Path | bytes, int]] = []
    size = 0
    for path in sorted(dist.rglob("*")):
        if path.is_file() and not path.is_symlink():
            mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
            files.append((f"./opt/qcyctl/{path.relative_to(dist)}", path, mode))
            size += path.stat().st_size
    links = [(f"./opt/qcyctl/{p.relative_to(dist)}", str(p.readlink()))
             for p in dist.rglob("*") if p.is_symlink()]
    files += [
        ("./usr/share/applications/qcyctl.desktop", ROOT / "packaging/linux/qcyctl.desktop", 0o644),
        ("./usr/share/icons/hicolor/256x256/apps/qcyctl.png", ROOT / "packaging/icons/qcyctl.png", 0o644),
        ("./usr/share/qcyctl/qcyctl-autostart.desktop",
         ROOT / "packaging/linux/qcyctl-autostart.desktop", 0o644),
    ]
    links += [("./usr/bin/qcyctl", "/opt/qcyctl/qcyctl"), ("./usr/bin/qcyctl-gui", "/opt/qcyctl/qcyctl-gui")]

    control = (
        "Package: qcyctl\n"
        f"Version: {version}\n"
        "Architecture: amd64\n"
        "Depends: libstdc++6, libgcc-s1, libgbm1\n"
        "Maintainer: qcyctl\n"
        "Section: sound\n"
        "Priority: optional\n"
        f"Installed-Size: {size // 1024}\n"
        "Description: Controle do fone QCY HT05 (ANC, EQ, toques, bateria)\n"
        " Projeto não oficial para Linux e Windows.\n"
    ).encode()
    deb = _ar([
        ("debian-binary", b"2.0\n"),
        ("control.tar.gz", _tar([("./control", control, 0o644)])),
        ("data.tar.gz", _tar(files, links)),
    ])
    Path(target).write_bytes(deb)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
