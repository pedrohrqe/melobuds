"""Ícone na bandeja: bateria no tooltip e troca rápida de modo."""

from __future__ import annotations

from PySide6.QtCore import QSettings
from PySide6.QtGui import QAction, QActionGroup
from PySide6.QtWidgets import QApplication, QMenu, QSystemTrayIcon

from .. import protocol as p
from ..device import DeviceState
from .controller import Controller
from .icons import tray_icon

QUICK_MODES = [
    ("Normal", p.NoiseMode(p.NoiseKind.NORMAL)),
    ("ANC (ruidoso)", p.NoiseMode(p.NoiseKind.ANC, "noisy", 3)),
    ("ANC (transporte)", p.NoiseMode(p.NoiseKind.ANC, "commute", 3)),
    ("ANC (interno)", p.NoiseMode(p.NoiseKind.ANC, "indoor", 3)),
    ("Transparência", p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=4)),
    ("Transparência (voz)", p.NoiseMode(p.NoiseKind.TRANSPARENCY, level=0)),
]


class Tray(QSystemTrayIcon):
    def __init__(self, ctl: Controller, window) -> None:
        super().__init__(tray_icon(False))
        self.ctl = ctl
        self.window = window
        menu = QMenu()
        self.battery_action = menu.addAction("Desconectado")
        self.battery_action.setEnabled(False)
        menu.addSeparator()

        self.mode_group = QActionGroup(menu)
        self.mode_actions: list[tuple[QAction, p.NoiseMode]] = []
        for label, mode in QUICK_MODES:
            act = menu.addAction(label)
            act.setCheckable(True)
            self.mode_group.addAction(act)
            act.triggered.connect(lambda _c=False, m=mode: ctl.run(lambda d: d.set_noise(m)))
            self.mode_actions.append((act, mode))
        menu.addSeparator()
        self.game_action = menu.addAction("Modo jogo")
        self.game_action.setCheckable(True)
        self.game_action.triggered.connect(lambda on: ctl.run(lambda d: d.set_game_mode(on)))
        menu.addSeparator()
        self.audio_action = menu.addAction("Conectar áudio", ctl.connect_audio)
        menu.addAction("Abrir janela", self.show_window)
        menu.addAction("Sair", self.quit)
        self.setContextMenu(menu)
        self._menu = menu
        self.activated.connect(self._activated)
        ctl.state_changed.connect(self.update_state)
        self.setToolTip("MeloBuds by @pedrohrqe — desconectado")

    def _activated(self, reason) -> None:
        if reason == QSystemTrayIcon.Trigger:
            if self.window.isVisible():
                self.window.hide()
            else:
                self.show_window()

    def show_window(self) -> None:
        self.window.show()
        self.window.raise_()
        self.window.requestActivate()

    def quit(self) -> None:
        self.window.setProperty("quitting", True)
        QApplication.quit()

    def update_state(self, s: DeviceState) -> None:
        self.setIcon(tray_icon(s.connected))
        for act, _ in self.mode_actions:
            act.setEnabled(s.connected)
        self.game_action.setEnabled(s.connected)
        self.audio_action.setEnabled(s.connected)
        if not s.connected:
            self.battery_action.setText("Desconectado")
            self.setToolTip("MeloBuds by @pedrohrqe — desconectado")
            return
        b = s.battery
        if not b:
            text = "Bateria: ?"
        elif str(QSettings("qcyctl", "qcyctl").value("ignore_case")).lower() == "true":
            text = f"L {b.left.label()} · R {b.right.label()}"   # "Ignorar caixa"
        else:
            text = f"L {b.left.label()} · R {b.right.label()} · caixa {b.case.label()}"
        self.battery_action.setText(text)
        mode = s.noise.label() if s.noise else "?"
        self.setToolTip(f"{s.pair_name or 'QCY HT05'}\n{text}\n{mode}")
        for act, m in self.mode_actions:
            act.setChecked(m == s.noise)
        if s.game_mode is not None:
            self.game_action.setChecked(s.game_mode)
