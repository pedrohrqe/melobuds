#!/usr/bin/env bash
# Gera AppImage, RPM e DEB em dist/packages a partir do código.
# Uso: packaging/linux/build-packages.sh   (execute na raiz do projeto)
set -euo pipefail

cd "$(dirname "$0")/../.."
PY=${PYTHON:-python3}
VERSION=$($PY -c "import tomllib;print(tomllib.load(open('pyproject.toml','rb'))['project']['version'])")
OUT=dist/packages
mkdir -p "$OUT"

# 1. Executáveis (pasta dist/qcyctl).
$PY -m PyInstaller --noconfirm --log-level WARN packaging/qcyctl.spec

# 2. RPM (rpmbuild).
if command -v rpmbuild > /dev/null; then
  rpmbuild -bb --quiet --define "_topdir $PWD/build/rpm" --define "_rpmdir $PWD/$OUT" \
    --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
    --define "_qcy_version $VERSION" --define "_qcy_root $PWD" packaging/linux/qcyctl.rpm.spec
else
  echo "rpmbuild ausente: RPM não gerado (Fedora: dnf install rpm-build; Ubuntu: apt install rpm)."
fi

# 3. DEB (script próprio, sem dpkg-deb).
$PY packaging/linux/make_deb.py "$VERSION" "$OUT/qcyctl_${VERSION}_amd64.deb"

# 4. AppImage (precisa do appimagetool no PATH ou em $APPIMAGETOOL).
APPIMAGETOOL=${APPIMAGETOOL:-$(command -v appimagetool || true)}
if [ -z "$APPIMAGETOOL" ]; then
  echo "appimagetool ausente: AppImage não gerado (o CI do GitHub gera)."
else
  APPDIR=build/AppDir
  rm -rf "$APPDIR"
  mkdir -p "$APPDIR/usr/lib"
  cp -a dist/qcyctl "$APPDIR/usr/lib/qcyctl"
  cp packaging/linux/qcyctl.desktop "$APPDIR/qcyctl.desktop"
  cp packaging/icons/qcyctl.png "$APPDIR/qcyctl.png"
  cp packaging/linux/AppRun "$APPDIR/AppRun"
  chmod +x "$APPDIR/AppRun"
  # APPIMAGE_EXTRACT_AND_RUN evita depender de FUSE no ambiente de build.
  ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" --no-appstream "$APPDIR" \
    "$OUT/qcyctl-$VERSION-x86_64.AppImage" > build/appimagetool.log 2>&1
fi

ls -la "$OUT"
