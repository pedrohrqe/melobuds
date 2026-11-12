"""Gera os ícones do aplicativo a partir de qcyctl.gui.icons.app_icon().

Uso: .venv/bin/python packaging/make_icons.py

Gera packaging/icons/qcyctl.png (256 px) e packaging/icons/qcyctl.ico.
O .ico contém vários tamanhos em PNG, para o Windows escolher o ideal.
"""

from __future__ import annotations

import os
import struct
import sys
from pathlib import Path

# Renderiza sem janela. Deve ser definido antes de criar a QGuiApplication.
os.environ["QT_QPA_PLATFORM"] = "offscreen"

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ / "src"))

from PySide6.QtCore import QBuffer, QIODevice
from PySide6.QtGui import QGuiApplication

from qcyctl.gui.icons import app_icon

PASTA = RAIZ / "packaging" / "icons"
TAMANHOS_ICO = (16, 24, 32, 48, 64, 128, 256)


def png_bytes(icone, tamanho: int) -> bytes:
    buffer = QBuffer()
    buffer.open(QIODevice.OpenModeFlag.WriteOnly)
    icone.pixmap(tamanho, tamanho).save(buffer, "PNG")
    return bytes(buffer.data())


def salva_ico(icone, caminho: Path) -> None:
    imagens = [(t, png_bytes(icone, t)) for t in TAMANHOS_ICO]
    cabecalho = struct.pack("<HHH", 0, 1, len(imagens))
    entradas = b""
    deslocamento = 6 + 16 * len(imagens)
    for tamanho, dados in imagens:
        dim = 0 if tamanho >= 256 else tamanho  # 0 significa 256 no formato ICO
        entradas += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(dados), deslocamento
        )
        deslocamento += len(dados)
    caminho.write_bytes(cabecalho + entradas + b"".join(d for _, d in imagens))


def main() -> int:
    app = QGuiApplication(sys.argv[:1])  # noqa: F841 (necessário para pintar com Qt)
    PASTA.mkdir(parents=True, exist_ok=True)
    icone = app_icon()

    png = PASTA / "qcyctl.png"
    if not icone.pixmap(256, 256).save(str(png), "PNG"):
        print(f"Falha ao gravar {png}", file=sys.stderr)
        return 1
    salva_ico(icone, PASTA / "qcyctl.ico")

    for arquivo in (png, PASTA / "qcyctl.ico"):
        print(f"{arquivo} ({arquivo.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
