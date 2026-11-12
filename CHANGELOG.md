# Registro de mudanças

Este arquivo segue o formato [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o [versionamento semântico](https://semver.org/lang/pt-BR/).

## [0.2.0] - 2026-11-12

### Alterado

- Reescrito em Rust. A interface usa Slint, com renderização por software. O app não usa mais Qt, Python nem OpenGL.
- Memória (PSS) com a janela aberta: de 165 MB para 20 MB. Na bandeja: 15 MB.
- Uso de CPU em repouso: de 7 % para cerca de 0 %. Medido no Fedora 44, com o fone conectado.
- Executável com cerca de 14 MB. Antes, o pacote tinha cerca de 250 MB.
- A aba ativa não tem borda e tem preenchimento mais forte.
- O anel de foco aparece só ao usar o teclado.
- O seletor "Ambiente" do ANC tem a mesma largura dos outros controles.
- Os nomes dos programas não mudam: `qcyctl-gui` e `qcyctl`.

### Removido

- Versão em Python, com PySide6 e QML. As versões 0.1.x continuam nas tags do repositório.

## [0.1.2] - 2026-11-12

### Melhorado

- Menos uso de CPU: as animações contínuas param quando a janela está fora de foco, minimizada ou na bandeja. Antes, o app desenhava até 144 quadros por segundo mesmo escondido.
- Linux: o app usa uma só conexão D-Bus com o BlueZ, sem abrir uma conexão nova a cada segundo.

## [0.1.1] - 2026-11-12

### Corrigido

- Linux: o app não abria em distribuições recentes, como o Fedora 44. O pacote levava uma `libstdc++` antiga, e o driver de vídeo do sistema não carregava. Agora o app usa a `libstdc++`, a `libgcc_s` e a `libgbm` do sistema.

## [0.1.0] - 2026-11-12

Primeira versão pública.

### Adicionado

- Interface gráfica escura (QML) com o desenho dos fones e a bateria de cada peça.
- Controle de ruído: ANC com 4 cenas e 3 níveis, e transparência com 7 níveis.
- Equalizador com 6 predefinições e modo personalizado de 10 bandas.
- Remapeamento de toques, modo jogo, modo sono e renomeação do fone.
- Ícone na bandeja do sistema.
- Conexão automática do áudio no Linux, com o perfil de música antes do perfil de chamadas.
- Aviso de Bluetooth desligado, com botão para ligar o Bluetooth no Linux.
- Opções "Ignorar caixa", "Iniciar com o sistema" e "Reduzir animações".
- Linha de comando `qcyctl`.
- Instaladores: RPM, DEB, AppImage e instalador para Windows.

[0.2.0]: https://github.com/pedrohrqe/melobuds/releases/tag/v0.2.0
[0.1.2]: https://github.com/pedrohrqe/melobuds/releases/tag/v0.1.2
[0.1.1]: https://github.com/pedrohrqe/melobuds/releases/tag/v0.1.1
[0.1.0]: https://github.com/pedrohrqe/melobuds/releases/tag/v0.1.0
