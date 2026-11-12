# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller em modo pasta (onedir): `qcyctl-gui` e `qcyctl` compartilham as bibliotecas.

A pasta dist/qcyctl é a base do AppImage, do RPM, do DEB e do instalador do Windows.
"""

import os
import sys

from PyInstaller.utils.hooks import collect_submodules

# Os backends do bleak são escolhidos em tempo de execução; o PyInstaller não os detecta.
ocultos = collect_submodules("bleak") + ["Registry", "Registry.Registry"]
if sys.platform == "win32":
    ocultos += collect_submodules("winrt")

# Módulos Qt grandes que o app não usa.
excluir = [
    "PySide6.QtWebEngineCore", "PySide6.QtWebEngineWidgets", "PySide6.Qt3DCore", "PySide6.QtMultimedia", "PySide6.QtPdf",
    "PySide6.QtCharts", "PySide6.QtDataVisualization", "tkinter",
]

icone = os.path.join(SPECPATH, "icons", "qcyctl.ico") if sys.platform == "win32" else None
src = os.path.join(SPECPATH, "..", "src")

# Interface QML (componentes, qmldir e fontes Inter) com o mesmo caminho do pacote.
qml = [(os.path.join(src, "qcyctl", "gui", "qml"), os.path.join("qcyctl", "gui", "qml"))]
# Módulos QML importados pela interface. O hook do PySide6 copia os plugins deles.
ocultos += ["PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtQuickControls2", "PySide6.QtSvg"]


# O hook do PySide6 copia todos os módulos QML instalados, e eles puxam bibliotecas
# grandes (só o WebEngine tem cerca de 200 MB). A interface usa QtQuick, Controls (Basic),
# Shapes e Window; o resto sai do pacote.
import re

DESCARTE = re.compile(
    r"WebEngine|WebView|WebChannel|WebSockets|Quick3D|Qt3D|Qt63D|Qt6Pdf|QtPdf|Graphs|Charts|"
    r"DataVisualization|Location|Positioning|Multimedia|SpatialAudio|Sensors|SerialPort|SerialBus|"
    r"RemoteObjects|Scxml|StateMachine|TextToSpeech|VirtualKeyboard|Qt6?Bluetooth|Qt6?Nfc|Qt6?Test|QuickTest|Designer|"
    r"Qt5Compat|LabsStyleKit|StyleKit|Lottie|QuickTimeline|QuickVectorImage|"
    r"Controls2?(Imagine|Material|Universal|Fusion|FluentWinUI3|Windows|macOS|iOS)|"
    r"Controls[/\\\\](Imagine|Material|Universal|Fusion|FluentWinUI3|Windows|macOS|iOS|designer)|"
    r"qtwebengine|resources[/\\\\]|translations[/\\\\]qtwebengine|libavcodec|libavformat|libavutil|"
    r"libswscale|libswresample",
    re.IGNORECASE,
)


# Linux: estas bibliotecas vêm do sistema. Os drivers Mesa do sistema exigem a
# libstdc++ do sistema; uma cópia mais antiga no pacote impede o OpenGL de iniciar.
DO_SISTEMA = {"libstdc++.so.6", "libgcc_s.so.1", "libgbm.so.1"}


def enxuga(toc):
    # Só arquivos do Qt: o filtro não toca em bleak, winrt ou no próprio app.
    toc = [item for item in toc if not ("PySide6" in item[0] and DESCARTE.search(item[0]))]
    if sys.platform.startswith("linux"):
        toc = [item for item in toc if os.path.basename(item[0]) not in DO_SISTEMA]
    return toc


def analise(script):
    a = Analysis(
        [os.path.join(SPECPATH, script)],
        pathex=[src],
        hiddenimports=ocultos,
        excludes=excluir,
        datas=qml,
        noarchive=False,
    )
    a.binaries = enxuga(a.binaries)
    a.datas = enxuga(a.datas)
    return a


a_gui = analise("run_gui.py")
a_cli = analise("run_cli.py")

exe_gui = EXE(
    PYZ(a_gui.pure), a_gui.scripts, [], exclude_binaries=True,
    name="qcyctl-gui", console=False, upx=False, icon=icone,
)
exe_cli = EXE(
    PYZ(a_cli.pure), a_cli.scripts, [], exclude_binaries=True,
    name="qcyctl", console=True, upx=False, icon=icone,
)

coll = COLLECT(
    exe_gui, a_gui.binaries, a_gui.datas,
    exe_cli, a_cli.binaries, a_cli.datas,
    upx=False, name="qcyctl",
)
