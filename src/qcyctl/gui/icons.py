"""Ícones desenhados em código (sem arquivos externos)."""

from __future__ import annotations

from PySide6.QtCore import QRectF, Qt
from PySide6.QtGui import QColor, QIcon, QPainter, QPainterPath, QPen, QPixmap

ACCENT = QColor("#2f80ed")


def _earbud(size: int, color: QColor, badge: str | None = None) -> QPixmap:
    pix = QPixmap(size, size)
    pix.fill(Qt.transparent)
    qp = QPainter(pix)
    qp.setRenderHint(QPainter.Antialiasing)
    s = size / 64
    # Corpo do fone: círculo com haste.
    path = QPainterPath()
    path.addEllipse(QRectF(8 * s, 6 * s, 34 * s, 34 * s))
    path.addRoundedRect(QRectF(30 * s, 26 * s, 12 * s, 32 * s), 6 * s, 6 * s)
    qp.fillPath(path.simplified(), color)
    qp.setBrush(QColor(255, 255, 255, 220))
    qp.setPen(Qt.NoPen)
    qp.drawEllipse(QRectF(17 * s, 15 * s, 16 * s, 16 * s))
    if badge:
        qp.setBrush(QColor("#27ae60") if badge == "on" else QColor("#95a5a6"))
        qp.setPen(QPen(Qt.white, 2 * s))
        qp.drawEllipse(QRectF(44 * s, 40 * s, 18 * s, 18 * s))
    qp.end()
    return pix


def app_icon() -> QIcon:
    icon = QIcon()
    for size in (16, 24, 32, 48, 64, 128, 256):
        icon.addPixmap(_earbud(size, ACCENT))
    return icon


def tray_icon(connected: bool) -> QIcon:
    icon = QIcon()
    color = ACCENT if connected else QColor("#7f8c8d")
    for size in (16, 22, 24, 32, 48, 64):
        icon.addPixmap(_earbud(size, color, "on" if connected else "off"))
    return icon
