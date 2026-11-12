"""Inicialização do app com o sistema.

Linux: arquivo .desktop em ~/.config/autostart. Windows: chave Run do usuário no registro.
O app inicia com --hidden (na bandeja, quando o sistema tem bandeja).
"""

from __future__ import annotations

import os
import shlex
import sys
from pathlib import Path

APP_NAME = "MeloBuds by @pedrohrqe"
ENTRY = "melobuds"                     # nome do arquivo .desktop e do valor no registro
RUN_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"


def command() -> list[str]:
    """Comando que abre este app: o executável empacotado, o script instalado ou o módulo."""
    if getattr(sys, "frozen", False):
        return [sys.executable, "--hidden"]
    script = Path(sys.argv[0]).resolve()
    if script.is_file() and os.access(script, os.X_OK) and script.suffix != ".py":
        return [str(script), "--hidden"]
    return [sys.executable, "-m", "qcyctl.gui.app", "--hidden"]


def _desktop_file() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "autostart" / f"{ENTRY}.desktop"


def is_enabled() -> bool:
    if sys.platform == "win32":
        import winreg

        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY) as key:
                winreg.QueryValueEx(key, ENTRY)
            return True
        except OSError:
            return False
    return _desktop_file().is_file()


def set_enabled(on: bool) -> None:
    """Liga ou desliga a inicialização com o sistema. Lança OSError se o sistema recusa."""
    if sys.platform == "win32":
        import subprocess
        import winreg

        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_KEY, 0, winreg.KEY_SET_VALUE) as key:
            if on:
                winreg.SetValueEx(key, ENTRY, 0, winreg.REG_SZ, subprocess.list2cmdline(command()))
            else:
                try:
                    winreg.DeleteValue(key, ENTRY)
                except FileNotFoundError:
                    pass
        return
    path = _desktop_file()
    if not on:
        path.unlink(missing_ok=True)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={APP_NAME}\n"
        "Comment=Inicia o controle do fone na bandeja\n"
        f"Exec={shlex.join(command())}\n"
        "Icon=qcyctl\n"
        "Terminal=false\n"
        "X-GNOME-Autostart-enabled=true\n",
        encoding="utf-8",
    )
