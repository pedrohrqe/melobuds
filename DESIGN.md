# DESIGN — MeloBuds by @pedrohrqe (qcyctl)

Fonte da verdade dos tokens: `ui/theme.slint`. Os componentes usam só os valores do `Theme`.

## Direção

Painel noturno: o painel de instrumentos de um carro à noite. O fundo é azul-escuro quase preto, e a luz de fundo é azul-gelo, a cor do LED do estojo. Modo de superfície: Operate (o usuário completa uma tarefa).

- Elemento de assinatura: o herói com os fones e o estojo desenhados em vetor. A bateria de cada peça fica acima do desenho.
- Ao conectar, os medidores de bateria fazem uma varredura de ignição (0 → 100 → valor real).
- Ondas atrás dos fones mostram o modo de ruído. Em ANC, as ondas contraem. Em transparência, as ondas expandem na cor de acento. Em normal, as ondas ficam paradas.
- O resto da tela fica quieto: cartões neutros, acento só no estado ativo e no foco.

## Cor

As cores usam o formato `#RRGGBBAA`, o formato do Slint.

| Token | Valor | Uso |
|---|---|---|
| `bg` | `#0B0D10` | Fundo da janela |
| `surface` | `#14171B` | Cartões |
| `surface-hi` | `#1C2026` | Hover, popup |
| `surface-press` | `#232831` | Pressionado |
| `hairline` | `#FFFFFF12` | Divisórias |
| `edge-light` | `#FFFFFF1C` | Luz na borda superior dos cartões e bordas de controles |
| `text` | `#ECEDEE` | Texto principal |
| `text2` | `#A1A7AD` | Texto secundário (7,3:1 sobre `surface`) |
| `text3` | `#6E747B` | Só para desativado e decoração |
| `accent` | `#8CCBFF` | Estado ativo, foco (10:1 sobre `surface`) |
| `accent-soft` | `#8CCBFF26` | Fundo de item ativo |
| `accent-ink` | `#0B1620` | Texto sobre `accent` |
| `ok` / `warn` / `bad` | `#4ADE80` / `#FBBF24` / `#F87171` | Bateria: acima de 30 %, de 15 a 30 %, abaixo de 15 % |
| `bud-top` / `bud-bottom` / `tip` | `#2B3038` / `#101216` / `#3A4048` | Desenho dos fones |

Um rótulo de texto sempre acompanha a cor da bateria.

## Tipografia

Inter e Inter Display (licença OFL), embutidas em `ui/fonts`.

| Token | Tamanho | Uso |
|---|---|---|
| `size-title` | 26 | Nome do fone (Inter Display, tracking −0,4) |
| `size-readout` | 22 | Percentual da bateria (números tabulares) |
| `size-heading` | 15 | Títulos dos cartões |
| `size-body` | 13 | Corpo e controles |
| `size-label` | 11 | Rótulos e notas |

## Espaço e forma

- Espaçamento base 4: `s1` 4, `s2` 8, `s3` 12, `s4` 16, `s5` 20, `s6` 24, `s8` 32.
- Raios: `radius` 14 (cartões) e `radius-sm` 9 (controles).
- Altura dos controles: `control` 36.
- Janela: largura 940, sem rolagem. Coluna esquerda de 400 px (cabeçalho, herói, Ruído). Coluna direita com abas (Equalizador, Toques, Dispositivo) e um painel único. A altura acompanha o conteúdo e reserva o espaço do maior painel; o gráfico do EQ estica para preencher o painel.

## Movimento

| Token | Valor | Uso |
|---|---|---|
| `instant` | 80 ms | Saída de popup |
| `fast` | 180 ms | Cor, ícone, chip |
| `normal` | 350 ms | Troca de aba, entrada |
| `slow` | 600 ms | Varredura da bateria |
| `out-cubic` | cubic-bezier(0,215; 0,61; 0,355; 1) | Curva padrão: pílula do controle segmentado, interruptor, polegar do EQ, barra de bateria |
| `in-out-cubic` | cubic-bezier(0,645; 0,045; 0,355; 1) | Laços de pulso e ondas |

- Ruído, abas e painel entram uma vez, com intervalo de 60 ms (opacidade e y de 16 px).
- Troca de aba: fade e deslize de 24 px na direção da troca.
- Pressionar um botão aplica escala 0,97 no evento de pressão.
- Movimento reduzido (`Theme.reduce`): `Theme.d(ms)` limita as durações a 150 ms, e as ondas e os pulsos ficam parados.
- Animações infinitas rodam só quando a janela está visível e em foco (`Theme.loop`).

## Componentes

Cada componente fica em `ui/<component>.slint`, com nome em kebab-case.

`bluetooth-off` (aviso de Bluetooth desligado: símbolo cortado por um traço; ao ligar, um anel pulsa uma vez), `card`, `segmented-control`, `chip`, `toggle`, `button`, `step-slider`, `select`, `field`, `info-row`, `toast`, `focus-ring`, `icons` (traços do Lucide, renderizados como SVG), `battery-readout`, `earbud`, `case-shape`, `noise-waves`, `earbuds-hero`, e os painéis `noise-panel`, `eq-panel`, `keys-panel` e `device-panel`.
