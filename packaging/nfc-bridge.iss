#ifndef ProductSlug
  #define ProductSlug "sync"
#endif
#ifndef AppVersion
  #define AppVersion "0.2.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef UniversalMode
  #define UniversalMode "0"
#endif
#ifndef InstalledBy
  #define InstalledBy "Standalone"
#endif

#if ProductSlug == "driver"
  #define MyAppGuid "{8E9B3B12-4F51-4A84-9F1B-A2E804B7D101}"
  #define MyAppName "NFC Bridge Driver"
  #define MyDefaultDir "{autopf}\NFC Bridge\Bridge Driver"
  #define MyInstalledExe "nfc-bridge-driver.exe"
  #define MyUninstIconIdx "1"
  #define MyPrivs "admin"
#elif ProductSlug == "tools"
  #define MyAppGuid "{8E9B3B12-4F51-4A84-9F1B-A2E804B7D103}"
  #define MyAppName "NFC Bridge Tools"
  #define MyDefaultDir "{autopf}\NFC Bridge\Bridge Tools"
  #define MyInstalledExe "NFC Bridge Tools.exe"
  #define MyUninstIconIdx "2"
  #define MyPrivs "admin"
#else
  #define MyAppGuid "{8E9B3B12-4F51-4A84-9F1B-A2E804B7D102}"
  #define MyAppName "NFC Bridge Sync"
  #define MyDefaultDir "{autopf}\NFC Bridge\Bridge Sync"
  #define MyInstalledExe "NFC Bridge Sync.exe"
  #define MyUninstIconIdx "2"
  #define MyPrivs "admin"
#endif
#define MyAppId "{" + MyAppGuid

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#AppVersion}
AppVerName={#MyAppName} {#AppVersion}
AppPublisher=HectorZaGa
VersionInfoVersion={#AppVersion}.0
VersionInfoProductVersion={#AppVersion}
VersionInfoCompany=HectorZaGa
VersionInfoDescription={#MyAppName} Setup
DefaultDirName={#MyDefaultDir}
DefaultGroupName=NFC Bridge
UsePreviousAppDir=yes
DisableDirPage=auto
DisableProgramGroupPage=yes
AllowNoIcons=yes
PrivilegesRequired={#MyPrivs}
#if UniversalMode == "1"
ArchitecturesAllowed=x64compatible x86 arm64
ArchitecturesInstallIn64BitMode=x64compatible arm64
#elif Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#elif Arch == "x86"
ArchitecturesAllowed=x86
ArchitecturesInstallIn64BitMode=
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
WizardStyle=modern
WizardResizable=no
Compression=lzma2/ultra64
SolidCompression=yes
SetupIconFile={#SetupIcon}
UninstallDisplayIcon={app}\{#MyInstalledExe},{#MyUninstIconIdx}
UninstallDisplayName={#MyAppName}
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
ShowLanguageDialog=no
#if ProductSlug != "driver"
CloseApplications=yes
CloseApplicationsFilter={#MyInstalledExe},nfc-bridge-sync*.exe,nfc-bridge-tools*.exe
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
english.UpdatePrompt=An older version (%1 v%2) is already installed.%nDo you want to update to version %3?
spanish.UpdatePrompt=Ya hay una versión anterior instalada (%1 v%2).%n¿Deseas actualizar a la versión %3?
german.UpdatePrompt=Eine ältere Version (%1 v%2) ist bereits installiert.%nMöchten Sie auf Version %3 aktualisieren?
french.UpdatePrompt=Une version antérieure (%1 v%2) est déjà installée.%nVoulez-vous mettre à jour vers la version %3 ?
portuguese.UpdatePrompt=Já existe uma versão anterior instalada (%1 v%2).%nPretende atualizar para a versão %3?
brazilianportuguese.UpdatePrompt=Já há uma versão anterior instalada (%1 v%2).%nDeseja atualizar para a versão %3?

english.ReinstallPrompt=%1 v%2 is already installed on this PC.%nDo you want to reinstall or repair the current installation?
spanish.ReinstallPrompt=%1 v%2 ya está instalado en este equipo.%n¿Deseas reinstalar o reparar la instalación actual?
german.ReinstallPrompt=%1 v%2 ist bereits auf diesem PC installiert.%nMöchten Sie die aktuelle Installation neu installieren oder reparieren?
french.ReinstallPrompt=%1 v%2 est déjà installé sur cet ordinateur.%nVoulez-vous réinstaller ou réparer l'installation actuelle ?
portuguese.ReinstallPrompt=%1 v%2 já está instalado neste computador.%nPretende reinstalar ou reparar a instalação atual?
brazilianportuguese.ReinstallPrompt=%1 v%2 já está instalado neste computador.%nDeseja reinstalar ou reparar a instalação atual?

english.DowngradePrompt=A newer version (%1 v%2) is already installed.%nDo you want to continue anyway with version %3?
spanish.DowngradePrompt=Ya tienes instalada una versión más reciente (%1 v%2).%n¿Deseas continuar de todas formas con la versión %3?
german.DowngradePrompt=Eine neuere Version (%1 v%2) ist bereits installiert.%nMöchten Sie trotzdem mit Version %3 fortfahren?
french.DowngradePrompt=Une version plus récente (%1 v%2) est déjà installée.%nVoulez-vous tout de même continuer avec la version %3 ?
portuguese.DowngradePrompt=Já está instalada uma versão mais recente (%1 v%2).%nPretende continuar mesmo assim com a versão %3?
brazilianportuguese.DowngradePrompt=Já há uma versão mais recente instalada (%1 v%2).%nDeseja continuar mesmo assim com a versão %3?

[Files]
#if UniversalMode == "1"
Source: "{#PayloadExeArm64}"; DestDir: "{app}"; DestName: "{#MyInstalledExe}"; Check: IsArm64; Flags: ignoreversion
Source: "{#PayloadExeX64}"; DestDir: "{app}"; DestName: "{#MyInstalledExe}"; Check: IsX64Compatible and not IsArm64; Flags: ignoreversion
Source: "{#PayloadExeX86}"; DestDir: "{app}"; DestName: "{#MyInstalledExe}"; Check: not IsX64Compatible; Flags: ignoreversion
#else
Source: "{#PayloadExe}"; DestDir: "{app}"; DestName: "{#MyInstalledExe}"; Flags: ignoreversion
#endif

#if ProductSlug != "driver"
[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Icons]
Name: "{autoprograms}\NFC Bridge\{#MyAppName}"; Filename: "{app}\{#MyInstalledExe}"; Parameters: "--open-ui"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyInstalledExe}"; Parameters: "--open-ui"; Tasks: desktopicon
#endif

[Run]
#if ProductSlug == "driver"
Filename: "{app}\{#MyInstalledExe}"; Parameters: "--install-service"; StatusMsg: "Instalando servicio NFC Bridge Driver..."; Flags: runhidden waituntilterminated
#else
Filename: "{app}\{#MyInstalledExe}"; Parameters: "--open-ui"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
Filename: "{app}\{#MyInstalledExe}"; Parameters: "--open-ui"; Flags: nowait; Check: WizardSilent
#endif

#if ProductSlug == "driver"
[UninstallRun]
Filename: "{app}\{#MyInstalledExe}"; Parameters: "--uninstall-service"; Flags: runhidden waituntilterminated; RunOnceId: "RemoveNfcBridgeDriverService"
#endif

[Registry]
Root: HKA; Subkey: "Software\NFC Bridge\{#ProductSlug}"; ValueType: string; ValueName: "InstalledBy"; ValueData: "{#InstalledBy}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\NFC Bridge\{#ProductSlug}"; ValueType: string; ValueName: "Version"; ValueData: "{#AppVersion}"

[Code]
procedure RunHiddenCmd(const FileName, Params: string);
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant(FileName), Params, '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure StopRunningTargets;
begin
#if ProductSlug == "driver"
  RunHiddenCmd('{sys}\sc.exe', 'stop NfcBridgeDriver');
  Sleep(500);
  RunHiddenCmd('{sys}\taskkill.exe', '/IM nfc-bridge-driver.exe /F');
#elif ProductSlug == "tools"
  RunHiddenCmd('{sys}\taskkill.exe', '/IM "NFC Bridge Tools.exe" /F');
  RunHiddenCmd('{sys}\taskkill.exe', '/IM nfc-bridge-tools.exe /F');
#else
  RunHiddenCmd('{sys}\taskkill.exe', '/IM "NFC Bridge Sync.exe" /F');
  RunHiddenCmd('{sys}\taskkill.exe', '/IM nfc-bridge-sync.exe /F');
#endif
  Sleep(250);
end;

function CompareVersions(const V1, V2: string): Integer;
var
  A, B: string;
  P, N1, N2: Integer;
begin
  A := V1;
  B := V2;
  Result := 0;
  while (Result = 0) and ((A <> '') or (B <> '')) do
  begin
    P := Pos('.', A);
    if P > 0 then begin
      N1 := StrToIntDef(Copy(A, 1, P - 1), 0);
      Delete(A, 1, P);
    end else begin
      N1 := StrToIntDef(A, 0);
      A := '';
    end;

    P := Pos('.', B);
    if P > 0 then begin
      N2 := StrToIntDef(Copy(B, 1, P - 1), 0);
      Delete(B, 1, P);
    end else begin
      N2 := StrToIntDef(B, 0);
      B := '';
    end;

    if N1 < N2 then Result := -1
    else if N1 > N2 then Result := 1;
  end;
end;

function InitializeSetup(): Boolean;
var
  SubKey, InstalledVer, Msg: string;
  Cmp, Resp: Integer;
begin
  Result := True;
  if WizardSilent() then Exit;

  SubKey := 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#MyAppGuid}_is1';
  InstalledVer := '';
  if not RegQueryStringValue(HKCU, SubKey, 'DisplayVersion', InstalledVer) then
  begin
    if not RegQueryStringValue(HKLM, SubKey, 'DisplayVersion', InstalledVer) then
      InstalledVer := '';
  end;
  if InstalledVer = '' then Exit;

  Cmp := CompareVersions(InstalledVer, '{#AppVersion}');
  if Cmp < 0 then
  begin
    Msg := FmtMessage(CustomMessage('UpdatePrompt'), ['{#MyAppName}', InstalledVer, '{#AppVersion}']);
    Resp := MsgBox(Msg, mbConfirmation, MB_YESNO);
    if Resp <> IDYES then Result := False;
  end
  else if Cmp = 0 then
  begin
    Msg := FmtMessage(CustomMessage('ReinstallPrompt'), ['{#MyAppName}', '{#AppVersion}']);
    Resp := MsgBox(Msg, mbConfirmation, MB_YESNO or MB_DEFBUTTON2);
    if Resp <> IDYES then Result := False;
  end
  else
  begin
    Msg := FmtMessage(CustomMessage('DowngradePrompt'), ['{#MyAppName}', InstalledVer, '{#AppVersion}']);
    Resp := MsgBox(Msg, mbError, MB_YESNO or MB_DEFBUTTON2);
    if Resp <> IDYES then Result := False;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    StopRunningTargets;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    StopRunningTargets;
  end;
end;
