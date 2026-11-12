# Política de segurança

## Versões com suporte

Só a versão mais recente recebe correções de segurança.

## Relatar uma vulnerabilidade

Não abra uma issue pública para uma vulnerabilidade.

1. Abra um [relato privado](https://github.com/pedrohrqe/melobuds/security/advisories/new).
2. Descreva o problema, o impacto e os passos para reproduzir.

O mantenedor responde em até 7 dias.

## Escopo

- O app não usa rede. Ele se comunica com o fone por Bluetooth e com o sistema local, como o BlueZ por D-Bus no Linux.
- O comando `qcyctl dualboot` exige `sudo` e altera a chave de pareamento no BlueZ. Antes da mudança, ele faz uma cópia de segurança. Relate qualquer falha desse comando como vulnerabilidade.
- O app não envia comandos de firmware (OTA) ao fone.
