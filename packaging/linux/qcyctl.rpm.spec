# RPM a partir dos binários do cargo (dist/qcyctl). Gerado por build-packages.sh.
%global debug_package %{nil}
%global __strip /bin/true
%global __requires_exclude_from ^/opt/qcyctl/.*$
%global __provides_exclude_from ^/opt/qcyctl/.*$

Name:           qcyctl
Version:        %{_qcy_version}
Release:        1
Summary:        Controle do fone QCY HT05 (ANC, EQ, toques, bateria)
License:        MIT
BuildArch:      x86_64
AutoReqProv:    no
Requires:       fontconfig libxkbcommon

%description
Controle do fone QCY HT05 (MeloBuds ANC): ANC, transparência, equalizador,
modo jogo, toques, bateria e nome. Projeto não oficial.

%install
mkdir -p %{buildroot}/opt %{buildroot}%{_bindir} %{buildroot}%{_datadir}/applications \
         %{buildroot}%{_datadir}/icons/hicolor/256x256/apps %{buildroot}%{_datadir}/qcyctl
cp -a %{_qcy_root}/dist/qcyctl %{buildroot}/opt/qcyctl
ln -s /opt/qcyctl/qcyctl %{buildroot}%{_bindir}/qcyctl
ln -s /opt/qcyctl/qcyctl-gui %{buildroot}%{_bindir}/qcyctl-gui
install -m 644 %{_qcy_root}/packaging/linux/qcyctl.desktop %{buildroot}%{_datadir}/applications/
install -m 644 %{_qcy_root}/packaging/icons/qcyctl.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/
install -m 644 %{_qcy_root}/packaging/linux/qcyctl-autostart.desktop %{buildroot}%{_datadir}/qcyctl/

%files
/opt/qcyctl
%{_bindir}/qcyctl
%{_bindir}/qcyctl-gui
%{_datadir}/applications/qcyctl.desktop
%{_datadir}/icons/hicolor/256x256/apps/qcyctl.png
%{_datadir}/qcyctl/qcyctl-autostart.desktop
