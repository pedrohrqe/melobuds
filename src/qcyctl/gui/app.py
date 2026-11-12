"""Ponto de entrada da interface gráfica."""

from __future__ import annotations

import argparse
import asyncio
import logging
import signal
import sys
from pathlib import Path

from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle
from PySide6.QtWidgets import QApplication, QSystemTrayIcon
from qasync import QEventLoop

from .bridge import Backend
from .controller import Controller
from .icons import app_icon
from .tray import Tray

QML_DIR = Path(__file__).resolve().parent / "qml"


def load_window(backend: Backend) -> tuple[QQmlApplicationEngine, object]:
    """Carrega a janela QML. Falha com erro claro se o QML não carregar."""
    engine = QQmlApplicationEngine()
    engine.addImportPath(str(QML_DIR))
    engine.rootContext().setContextProperty("backend", backend)
    engine.load(str(QML_DIR / "Main.qml"))
    if not engine.rootObjects():
        raise RuntimeError("falha ao carregar a interface (Main.qml)")
    return engine, engine.rootObjects()[0]


def main() -> int:
    parser = argparse.ArgumentParser(prog="qcyctl-gui")
    parser.add_argument("--address", "-a", help="endereço BLE do fone (padrão: procura)")
    parser.add_argument("--hidden", action="store_true", help="inicia só na bandeja")
    parser.add_argument("--debug", action="store_true")
    args, qt_args = parser.parse_known_args()
    logging.basicConfig(
        level=logging.DEBUG if args.debug else logging.WARNING,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    # Cada consulta ao BlueZ abre e fecha uma conexão D-Bus. Ao fechar, o dbus_fast
    # registra como erro um pedido interno ainda pendente. O aviso não indica falha.
    logging.getLogger("dbus_fast.message_bus").setLevel(logging.CRITICAL)

    QQuickStyle.setStyle("Basic")
    app = QApplication([sys.argv[0], *qt_args])
    app.setApplicationName("qcyctl")
    app.setApplicationDisplayName("MeloBuds by @pedrohrqe")
    app.setDesktopFileName("qcyctl")
    app.setWindowIcon(app_icon())
    tray_ok = QSystemTrayIcon.isSystemTrayAvailable()
    app.setQuitOnLastWindowClosed(not tray_ok)

    loop = QEventLoop(app)
    asyncio.set_event_loop(loop)

    ctl = Controller(args.address)
    backend = Backend(ctl, tray_ok)
    _engine, window = load_window(backend)
    if tray_ok:
        tray = Tray(ctl, window)
        tray.show()
    if not (args.hidden and tray_ok):
        window.show()

    closing = asyncio.Event()
    app.aboutToQuit.connect(closing.set)
    # kill/Ctrl+C: fecha com a desconexão do fone, para o sistema não manter o elo BLE aberto.
    for sig in (signal.SIGTERM, signal.SIGINT):
        try:
            loop.add_signal_handler(sig, app.quit)
        except (NotImplementedError, RuntimeError, ValueError):
            signal.signal(sig, lambda *_: app.quit())
    with loop:
        ctl.start()
        loop.run_until_complete(closing.wait())
        loop.run_until_complete(ctl.stop())
    return 0


if __name__ == "__main__":
    sys.exit(main())
