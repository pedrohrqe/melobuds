# Componentes de terceiros

O app inclui ou usa estes componentes. Cada um mantém a própria licença.

| Componente | Uso | Licença |
|---|---|---|
| [Qt for Python (PySide6)](https://doc.qt.io/qtforpython-6/) | Interface gráfica | LGPL-3.0 |
| [bleak](https://github.com/hbldh/bleak) | Bluetooth Low Energy | MIT |
| [qasync](https://github.com/CabbageDevelopment/qasync) | Integração de asyncio com o Qt | BSD-2-Clause |
| [dbus-fast](https://github.com/Bluetooth-Devices/dbus-fast) | D-Bus (BlueZ) no Linux | MIT |
| [python-registry](https://github.com/williballenthin/python-registry) | Leitura do registro do Windows (dual boot, opcional) | Apache-2.0 |
| [Inter](https://rsms.me/inter/) | Fontes da interface | SIL Open Font License 1.1 |
| [Lucide](https://lucide.dev) | Geometria dos ícones | ISC |

Os instaladores trazem o PySide6 como bibliotecas dinâmicas separadas, na pasta do app. Você pode substituir essas bibliotecas, como a LGPL exige.
