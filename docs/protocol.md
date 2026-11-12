# Protocolo do QCY HT05 (MeloBuds ANC)

Este documento registra o protocolo descoberto por engenharia reversa do app QCY Android
(`com.qcymall.googleearphonesetup`) e por testes com o fone real.

## Endereços

O fone tem dois endereços. O endereço BLE é o endereço clássico com o primeiro byte trocado.

| Transporte | Exemplo | Uso |
|---|---|---|
| Bluetooth clássico (BR/EDR) | `84:AC:60:11:22:33` | Áudio (A2DP, HFP), SPP (`00001101`) |
| BLE | `C4:AC:60:11:22:34` | Controle (GATT). Não exige pareamento. |

O anúncio BLE tem manufacturer data `0x521c`. O payload contém o endereço clássico em ordem inversa (`33221160ac84`).

## Comportamento do anúncio BLE

- O fone anuncia BLE em modo de pareamento e por cerca de 30 a 90 s depois de conectar o áudio clássico.
- Depois dessa janela, o anúncio para. Uma conexão BLE aberta dentro da janela continua ativa (testado por mais de 3 min).
- O controle BLE não exige pareamento nem autenticação.

Manufacturer data (company ID `0x521C`, payload sem o ID):

| Offset | Conteúdo |
|---|---|
| 0–1 | vendorId (modelo), big endian. HT05 = `0x4D41` (família "WQ") |
| 5, 6, 7 | bateria L, R, caixa (bit 7 = carregando, bits 0–6 = %) |
| 12, 11, 13, 16, 15, 14 | MAC clássico, nesta ordem |

## GATT

Só o serviço `0000a001` é usado para controle. O serviço `00007033` repete os UUIDs `1001`/`1002` e é do SDK do chip (OTA). Por isso, procure as characteristics dentro do serviço `a001`.

| Characteristic | Propriedades | Conteúdo |
|---|---|---|
| `0007` | read | versão: `L.maj, L.min, L.patch, R.maj, R.min, R.patch` (ex.: `04 00 00 04 00 00` = 4.0.0) |
| `0008` | read, notify | bateria `[L, R, caixa]` (bit 7 = carregando; 0 = desconhecido) |
| `0009` | read | `"WQ00"` (identificador da família) |
| `000B` | read, write-no-resp | EQ: `[tipo, 10 ganhos int8 em dB]` |
| `000D` | read, write-no-resp | toques: pares `[evento, ação]`; escrita de um par por vez |
| `1001` | write-no-resp | comandos (frames TLV) |
| `1002` | read, notify | respostas e mudanças de estado (frames TLV) |
| `1003` | read | `"WQ00" 00 02 04 03 05 07 06` (incerto) |

## Frames TLV (`1001` e `1002`)

```
FF <len> [<id> <n> <payload n bytes>]...
```

- `len` = tamanho total − 2. Não há checksum nem criptografia.
- Leitura: envie `FF 03 FE 01 <id>`. A resposta chega em `1002` com o TLV `<id>`.
- O fone confirma cada escrita com o mesmo TLV em `1002`.
- A leitura direta de `1002` retorna um frame com vários TLVs de estado.

| ID | Nome | Payload | Notas |
|---|---|---|---|
| `0x01` | reset de configurações | — | `FF 02 01 00` |
| `0x03` | reset de fábrica | — | `FF 02 03 00`. Não usar em testes. |
| `0x06` | detecção de ouvido | 1 byte | lido: `01` |
| `0x08` | volume | `L, R, máx` | lido: `7F 7F 0F` |
| `0x09` | modo jogo | `01` liga, `02` desliga | confirmado |
| `0x0C` | modo de ruído | 1 byte (tabela abaixo) | confirmado |
| `0x10` | modo sono | `01` liga, `02` desliga | desativa os toques até guardar na caixa |
| `0x16` | balanço | 0–100 | lido: `32` (50) |
| `0x18` | nome Bluetooth | UTF-8, até 32 bytes | resposta com 32 bytes completados com zeros |
| `0xFE` | pedido de leitura | 1 byte: id | |

### Modo de ruído (`0x0C`)

Valores do painel oficial do modelo 19777 (firmware ≥ 2.0.9):

| Valor | Modo |
|---|---|
| `0x02` | Normal (ANC e transparência desligados) |
| `0x41`–`0x43` | ANC Interno, níveis 1–3 |
| `0x51`–`0x53` | ANC Transporte, níveis 1–3 |
| `0x61`–`0x63` | ANC Ruidoso, níveis 1–3 (padrão `0x63`) |
| `0x71` | ANC Anti-vento |
| `0xA0` | Transparência com realce de voz |
| `0xA1`–`0xA6` | Transparência, níveis 1–6 (padrão `0xA4`) |

### EQ (`000B`)

- Tipo `0x00`–`0x05`: preset interno (Padrão, Pop, Graves, Rock, Suave, Clássico). O firmware ignora os ganhos e devolve zeros.
- Tipo `0xFF`: personalizado. Ganhos int8 de −8 a +8 dB nas bandas 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz.

### Toques (`000D`)

| Evento | Gesto |
|---|---|
| 1 / 2 | toque simples esquerdo / direito |
| 3 / 4 | toque duplo esquerdo / direito |
| 5 / 6 | toque triplo esquerdo / direito |

Ações: 0 nenhuma, 1 play/pausa, 2 faixa anterior, 3 próxima faixa, 4 assistente, 5 volume +, 6 volume −, 7 modo jogo, 11 alternar ANC.

Em chamadas, os gestos são fixos: toque duplo atende ou desliga; manter 1,5 s recusa.

## Caixa de carga

A caixa não tem comandos próprios. Ela informa só a carga (byte 3 da characteristic `0008` e do anúncio), e só quando um fone está dentro dela com a tampa aberta. O LED da caixa não é controlável.

## Origem dos dados

- Código do app QCY 4.0.7 (Google Play), classes `QCYHeadsetClient`, `DataAnalyse`, `DataBean`, `EQDataBean`, `PairnameDataBean`.
- Painel de controle do servidor QCY: `POST https://api.watch.qcy.com/product/findControlPanels` com `modelId=19777`.
- Testes no fone real (firmware 4.0.0).
