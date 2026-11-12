# Product

<!-- impeccable:product-schema 1 -->

## Platform

Desktop (Linux: Fedora, Ubuntu; Windows 10+). Interface em Slint, com renderização por software. Não é web.

## Users

Donos do fone QCY HT05 (MeloBuds ANC) que usam PC. O app oficial existe só para celular. Muitos usuários não têm celular com o app, ou usam um PC com dual boot. Eles abrem o app para trocar o modo de ruído, o EQ ou os controles de toque, e para ver a bateria. As sessões são curtas e frequentes; o app costuma ficar na bandeja do sistema.

## Product Purpose

Controlar todas as funções do HT05 pelo PC: modo de ruído (cenas e níveis de ANC, níveis de transparência), EQ (predefinições e 10 bandas personalizadas), gestos de toque, modo jogo, modo sono, nome Bluetooth e bateria dos dois fones e do estojo. Sucesso: o app conecta sozinho ao abrir, e cada mudança faz efeito nos fones em menos de um segundo.

## Positioning

O único app de PC para o HT05. Ele usa o protocolo BLE próprio dos fones, feito por engenharia reversa do app oficial. Por isso, não precisa de celular nem de conta. O app é autocontido: o pacote instala tudo, e nada mais precisa ser instalado.

## Constraints

- Pacotes autocontidos (AppImage, RPM, DEB, instalador do Windows). No Linux, o app usa só fontconfig e libxkbcommon, que já vêm nos desktops.
- O controle BLE não exige pareamento. O áudio usa Bluetooth clássico e deve ficar conectado.
- Os fones anunciam BLE por cerca de 60 s depois que o áudio conecta. O app pode reconectar o áudio por pouco tempo para alcançá-los.
- Nunca envie comandos de firmware (OTA) nem de restauração de fábrica.
- Idioma da interface: português do Brasil.

## Accessibility

Contraste de texto de pelo menos 4,5:1. A cor nunca indica um estado sozinha: a bateria sempre mostra um número. Operação por teclado em todos os controles. Opção "reduzir animações", além da configuração de animações do sistema.
