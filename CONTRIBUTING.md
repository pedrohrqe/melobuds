# Como contribuir

Obrigado pelo interesse no projeto. Este guia explica como relatar um problema e como enviar uma mudança.

## Relatar um problema

1. Procure o problema nas [issues](https://github.com/pedrohrqe/melobuds/issues).
2. Se ele não existe, abra uma issue com o modelo "Relato de bug".
3. Informe o sistema operacional, a versão do app e o modelo do fone.
4. Anexe o log: execute `qcyctl-gui --debug` e copie as linhas do momento da falha.

Remova do log os endereços Bluetooth que você não quer publicar.

## Ambiente de desenvolvimento

Requisitos: Rust no canal `stable`, instalado pelo rustup. No Linux, também BlueZ 5.50 ou mais recente e o pacote de desenvolvimento do fontconfig:

- Fedora: `sudo dnf install fontconfig-devel`.
- Ubuntu e Debian: `sudo apt install libfontconfig1-dev pkg-config`.

1. Clone o repositório.
2. Abra a interface: `cargo run`.
3. Abra a interface com log de depuração: `cargo run -- --debug`.
4. Execute a linha de comando: `cargo run --bin qcyctl -- status`.

## Antes de enviar um pull request

1. Execute `cargo clippy --all-targets`.
2. Execute `cargo test`.
3. Se a mudança altera a interface, siga os tokens de `DESIGN.md` e de `ui/theme.slint`. Anexe uma captura de tela, gerada com `cargo run --example shot -- <pasta>`.
4. Se a mudança altera o protocolo, atualize `docs/protocol.md` e os testes de `src/protocol.rs`.

## Regras do código

- Escreva os comentários e as mensagens de commit em português.
- Mantenha os identificadores em inglês.
- Use o padrão [Conventional Commits](https://www.conventionalcommits.org/pt-br/) nas mensagens de commit.
- Faça um pull request para cada assunto.

## Segurança do fone

Não envie comandos OTA (atualização de firmware) nem comandos de restauração de fábrica ao fone. O projeto não aceita código que envia esses comandos. Um comando incorreto pode danificar o fone.

## Outros modelos de fone

O app foi testado só com o QCY HT05 (MeloBuds ANC). Para adicionar outro modelo, abra uma issue com o `vendorId` do anúncio BLE (`qcyctl scan`) antes de escrever o código.

## Licença

Ao contribuir, você aceita que a sua contribuição use a licença MIT do projeto.
