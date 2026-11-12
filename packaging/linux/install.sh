#!/usr/bin/env bash
# Instala o qcyctl para o usuário atual, sem sudo.
# Uso: packaging/linux/install.sh [--autostart]
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AUTOSTART=0

for arg in "$@"; do
    case "$arg" in
        --autostart) AUTOSTART=1 ;;
        -h|--help)
            echo "Uso: $0 [--autostart]"
            exit 0
            ;;
        *)
            echo "Opção desconhecida: $arg" >&2
            exit 1
            ;;
    esac
done

# Instala o pacote. Com pipx, o comando fica em ~/.local/bin.
cd "$RAIZ"
if command -v pipx >/dev/null 2>&1; then
    pipx install --force .
else
    python3 -m pip install --user --upgrade .
fi

# Registra o atalho de aplicativo e o ícone.
APPS="$HOME/.local/share/applications"
ICONES="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$APPS" "$ICONES"
install -m 0644 "$RAIZ/packaging/linux/qcyctl.desktop" "$APPS/qcyctl.desktop"
install -m 0644 "$RAIZ/packaging/icons/qcyctl.png" "$ICONES/qcyctl.png"

# Inicia com a sessão, se pedido.
if [ "$AUTOSTART" -eq 1 ]; then
    mkdir -p "$HOME/.config/autostart"
    install -m 0644 "$RAIZ/packaging/linux/qcyctl-autostart.desktop" \
        "$HOME/.config/autostart/qcyctl-autostart.desktop"
fi

# Atualiza os caches, se as ferramentas existirem.
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" || true

echo "Instalação concluída. Comando: qcyctl-gui"
