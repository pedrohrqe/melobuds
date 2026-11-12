# Product

<!-- impeccable:product-schema 1 -->

## Platform

desktop (Linux: Fedora, Ubuntu; Windows 10+). Qt Quick (PySide6). Not web.

## Users

Owners of the QCY HT05 (MeloBuds ANC) earbuds who use a PC. The official app exists only for phones. Many users have no phone with the app, or use a dual-boot PC. They open the app to change the noise mode, the EQ, or the touch controls, and to check the battery. Sessions are short and frequent; the app usually lives in the system tray.

## Product Purpose

Control every function of the HT05 from the PC: noise mode (ANC scenes and levels, transparency levels), EQ (presets and 10 custom bands), touch gestures, game mode, sleep mode, Bluetooth name, and battery of both earbuds and the case. Success: the app connects by itself when it opens, and each change takes effect on the earbuds in under a second.

## Positioning

The only PC app for the HT05. It speaks the earbuds' own BLE protocol (reverse engineered from the official app), so it needs no phone and no account. It is self-contained: the package installs everything; nothing else must be installed.

## Constraints

- Self-contained packages (AppImage, RPM, DEB, Windows installer). No system dependencies declared.
- BLE control needs no pairing. Audio uses classic Bluetooth and must stay connected.
- The earbuds advertise BLE only for about 60 s after the audio connects; the app may briefly reconnect the audio to reach them.
- Never send firmware/OTA or factory-reset commands.
- UI language: Brazilian Portuguese.

## Accessibility

Text contrast at least 4.5:1. Color never carries a state alone (battery level always has a number). Keyboard operation of every control. A "reduce animations" option, plus the desktop's own animation setting.
