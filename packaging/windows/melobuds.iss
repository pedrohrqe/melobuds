; Instalador do Windows (Inno Setup 6). Entrada: dist\qcyctl (cargo build --release).
; Uso: iscc /DAppVersion=0.1.0 packaging\windows\melobuds.iss   (na raiz do projeto)
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6F0C1F4E-7B1D-4C55-9A0B-3B7D2C1E9A41}
AppName=MeloBuds by @pedrohrqe
AppVersion={#AppVersion}
AppPublisher=pedrohrqe
AppPublisherURL=https://github.com/pedrohrqe
AppSupportURL=https://github.com/pedrohrqe/melobuds/issues
DefaultDirName={autopf}\MeloBuds
DefaultGroupName=MeloBuds
DisableProgramGroupPage=yes
; Instala para o usuário atual, sem pedir permissão de administrador.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
SourceDir=..\..
OutputDir=dist\packages
OutputBaseFilename=qcyctl-{#AppVersion}-windows-setup
SetupIconFile=packaging\icons\qcyctl.ico
UninstallDisplayIcon={app}\qcyctl.ico
LicenseFile=LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[Files]
Source: "dist\qcyctl\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\MeloBuds"; Filename: "{app}\qcyctl-gui.exe"; IconFilename: "{app}\qcyctl.ico"
Name: "{autodesktop}\MeloBuds"; Filename: "{app}\qcyctl-gui.exe"; IconFilename: "{app}\qcyctl.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\qcyctl-gui.exe"; Description: "{cm:LaunchProgram,MeloBuds}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM qcyctl-gui.exe /F"; Flags: runhidden; RunOnceId: "FechaApp"

[Registry]
; Remove a inicialização com o sistema criada pelo app.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "melobuds"; Flags: dontcreatekey uninsdeletevalue
