; Spider PDF Manager installer (Inno Setup 6).
;
; Built by GitHub Actions (see .github/workflows/build.yml): Inno Setup is
; installed with Chocolatey, then ISCC compiles this script. The dist/ folder
; (gui exe + cli exe + OCR models + licences, staged by the workflow) is
; packaged together. Per-user install, no admin rights needed: the app lives
; in %LOCALAPPDATA%\Programs, shortcuts go to the user's Start menu and
; (optionally) desktop.
;
; Paths are relative to this script's folder (installer/).

#define AppName "Spider PDF Manager"
#define AppVersion "1.2"
#define AppExe "spider-pdf-manager.exe"

[Setup]
AppId={{C57CE41A-E45F-4F19-9A2D-EF02B4E9751D}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Spider
AppPublisherURL=https://github.com/seike8429/spider-pdf-manager
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
PrivilegesRequired=lowest
OutputDir=installer-out
OutputBaseFilename=SpiderPDFManager-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#AppExe}
SetupIconFile=..\assets\app-icon\spider-pdf-manager.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The app (dist/ is staged by the workflow: gui exe, cli exe, OCR models,
; licences, README).
Source: "..\dist\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
