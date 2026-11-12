# MeloBuds by @pedrohrqe (qcyctl)

[![ci](https://github.com/pedrohrqe/melobuds/actions/workflows/ci.yml/badge.svg)](https://github.com/pedrohrqe/melobuds/actions/workflows/ci.yml)
[![release](https://github.com/pedrohrqe/melobuds/actions/workflows/release.yml/badge.svg)](https://github.com/pedrohrqe/melobuds/actions/workflows/release.yml)
[![Licença: MIT](https://img.shields.io/badge/licen%C3%A7a-MIT-blue.svg)](LICENSE)

Controle não oficial para o fone QCY HT05 (MeloBuds ANC). Oferece uma interface gráfica, um ícone na bandeja do sistema e uma linha de comando.

Este projeto não tem afiliação com a QCY. Autor: [@pedrohrqe](https://github.com/pedrohrqe).

## Recursos

- ANC com cenas: Interno, Transporte, Ruidoso e Anti-vento. Cada cena tem nível de 1 a 3.
- Transparência com níveis de 0 a 6. O nível 0 ativa o realce de voz.
- Equalizador com 6 predefinições e 1 modo personalizado de 10 bandas, de -8 dB a +8 dB.
- Modo jogo, com baixa latência.
- Modo sono.
- Remapeamento de toques: toque simples, duplo e triplo, em cada lado.
- Bateria dos fones esquerdo e direito, e do estojo.
- Renomeação do fone.
- Ícone na bandeja do sistema.

## Interface

A janela tem tema escuro, duas colunas e nenhuma rolagem:

- No topo, o desenho dos fones e do estojo, com a bateria de cada peça acima do desenho. Ondas atrás dos fones mostram o modo de ruído.
- Abaixo do desenho, o cartão Ruído. Na coluna direita, as abas Equalizador, Toques e Dispositivo mostram um painel de cada vez.
- Fechar a janela esconde o app na bandeja. Use "Sair" no menu da bandeja para encerrar.

Na aba Dispositivo:

- "Ignorar caixa" esconde a caixa e descarta as informações dela.
- "Iniciar com o sistema" abre o app na bandeja quando você entra no sistema. No Linux, o app cria `~/.config/autostart/melobuds.desktop`. No Windows, o app usa a chave `Run` do usuário no registro.

Se o Bluetooth do sistema está desligado, a janela mostra um aviso e o app não procura o fone. No Linux, o botão "Ligar Bluetooth" liga o adaptador.

Para reduzir as animações, ative "Reduzir animações" no cartão Dispositivo. No GNOME, o app segue a configuração de animações do sistema até você mudar este interruptor.

## Requisitos

Linux:

- BlueZ 5.50 ou mais recente.
- Python 3.11 ou mais recente.
- GNOME: instale a extensão AppIndicator para exibir o ícone na bandeja.

Windows:

- Windows 10 versão 1809 ou mais recente.
- Adaptador Bluetooth com suporte a BLE.

## Instalação

Baixe o arquivo do seu sistema na página de [releases](https://github.com/pedrohrqe/melobuds/releases/latest). Os instaladores trazem tudo o que o app precisa; você não instala Python nem outra dependência.

| Sistema | Arquivo | Como instalar |
|---|---|---|
| Fedora, openSUSE | `qcyctl-<versão>-1.x86_64.rpm` | `sudo dnf install ./qcyctl-*.rpm` |
| Ubuntu, Debian, Mint | `qcyctl_<versão>_amd64.deb` | `sudo apt install ./qcyctl_*.deb` |
| Qualquer Linux | `qcyctl-<versão>-x86_64.AppImage` | `chmod +x qcyctl-*.AppImage` e execute o arquivo |
| Windows 10 e 11 | `qcyctl-<versão>-windows-setup.exe` | Execute o instalador |
| Windows, sem instalar | `qcyctl-<versão>-windows-portable.zip` | Extraia e execute `qcyctl-gui.exe` |

Depois da instalação, abra "MeloBuds by @pedrohrqe" no menu de aplicativos.

O arquivo `SHA256SUMS.txt` do release tem as somas de verificação. O instalador do Windows não tem assinatura digital; por isso o SmartScreen pode mostrar um aviso na primeira execução.

### A partir do código

1. Clone o repositório.
2. Execute `python -m venv .venv`.
3. Execute `.venv/bin/pip install -e .`.
4. Execute `.venv/bin/qcyctl-gui`.

Para gerar os instaladores do Linux, execute `packaging/linux/build-packages.sh`.

## Uso da linha de comando

```
qcyctl status                           # bateria, modos, EQ e toques
qcyctl noise anc noisy 3                # ANC, cena Ruidoso, nível 3
qcyctl noise transparency 4             # transparência, nível 4
qcyctl noise off                        # desliga ANC e transparência
qcyctl eq Rock                          # predefinição Rock
qcyctl eq --gains 3,2,1,0,0,0,0,0,1,2   # EQ personalizado com 10 valores
qcyctl game on                          # modo jogo ligado
qcyctl key 6 11                         # remapeia o toque 6 para a ação 11
qcyctl rename "Meu HT05"                # muda o nome Bluetooth do fone
qcyctl scan                             # lista fones próximos
sudo qcyctl dualboot                    # copia a chave de pareamento do Windows
```

Nomes de cenas na linha de comando: `indoor` (Interno), `commute` (Transporte), `noisy` (Ruidoso) e `wind` (Anti-vento).

Predefinições de EQ: Padrão, Pop, Graves, Rock, Suave e Clássico.

Os valores aceitos para `key` estão em [docs/protocol.md](docs/protocol.md).

## Conexão com o fone

O controle por BLE não exige pareamento.

O HT05 anuncia sinal BLE por cerca de 60 segundos depois que o áudio conecta. Ele também anuncia quando sai do estojo em modo de pareamento.

O aplicativo busca o fone sem parar e mantém a conexão BLE.

Se o aplicativo não encontrar o fone:

1. Coloque os fones no estojo.
2. Retire os fones do estojo.

## Dual boot (Linux e Windows)

Os dois sistemas usam o mesmo adaptador Bluetooth. Cada pareamento gera uma nova chave. Por isso, o pareamento feito no Windows invalida a chave do Linux.

Para usar o fone nos dois sistemas:

1. Pareie os fones no Linux.
2. Inicie o Windows e pareie os fones lá.
3. Volte ao Linux.
4. Monte a partição do Windows somente leitura: `udisksctl mount -b /dev/<partição> -o ro`.
5. Instale o extra de dual boot: `uv pip install -e '.[dualboot]'`.
6. Execute `sudo .venv/bin/qcyctl dualboot`.

O comando lê somente o registro do Windows. Antes de alterar a chave, ele faz uma cópia de segurança do arquivo de informações do BlueZ. Depois, ele reinicia o serviço `bluetooth`.

Para comparar as chaves sem alterar nada, execute `sudo .venv/bin/qcyctl dualboot --dry-run`.

## Protocolo

A documentação do protocolo BLE está em [docs/protocol.md](docs/protocol.md).

## Aviso

Não envie comandos OTA (atualização de firmware) ao fone. Um comando incorreto pode danificar o fone.

## Contribuição

Leia [CONTRIBUTING.md](CONTRIBUTING.md) e o [código de conduta](CODE_OF_CONDUCT.md). Relate vulnerabilidades como descreve [SECURITY.md](SECURITY.md). As mudanças de cada versão estão em [CHANGELOG.md](CHANGELOG.md).

## Licença e marcas

O código usa a licença [MIT](LICENSE). Os componentes de terceiros estão em [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

QCY e MeloBuds são marcas dos respectivos donos. Este projeto é independente e não tem afiliação com a QCY, nem apoio dela.

Autor: [@pedrohrqe](https://github.com/pedrohrqe).
