; officework の Windows の入れ物(Inno Setup)。
;
;   ISCC.exe /DVersion=0.1.0-alpha /DSrc=<包んだフォルダ> packaging\windows\officework.iss
;
; **zip をやめてこれにした**(発注者 2026-08-17「Windows は zip では
; よくないのでは」)。zip では
;   - スタートメニューに出ない
;   - アンインストールできない
;   - .xlsx / .docx を関連付けられない
;   - 利用者に「展開して officework.exe を探して起こす」をさせる
; の4つが全部そのままだった。
;
; **署名はしていない**(発注者 2026-08-17「安定したら Microsoft Store から
; 出せばいい」)。それまで SmartScreen が出るので、Releases の説明で
; 「詳細情報 → 実行」と正直に案内する。
;
; **管理者権限を求めない**(PrivilegesRequired=lowest)。会社の PC でも
; 試せるようにするため — アルファで一番効く。入る先は
; %LOCALAPPDATA%\Programs\officework。

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Src
  #define Src "..\out\windows"
#endif

[Setup]
AppId={{8F3A6C21-7B4E-4E2A-9C1D-0A5E2B7D3F60}
AppName=officework
AppVersion={#Version}
AppPublisher=aiseed-dev
AppPublisherURL=https://github.com/aiseed-dev/officework
DefaultDirName={autopf}\officework
DefaultGroupName=officework
; 入れ先を選ばせない(迷う所を減らす)。変えたい人は /DIR= で渡せる
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\out
OutputBaseFilename=officework-{#Version}-windows-x86_64-setup
SetupIconFile=..\icons\officework.ico
UninstallDisplayIcon={app}\officework.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; 同梱 Python があるので素で 200MB ほどになる
DiskSpanning=no

[Languages]
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked
; **関連付けは既定で切っておく。** いま使っている表計算やワープロを
; 黙って横取りしない(入れただけで既定が変わるのは事故のもと)
Name: "assocxlsx"; Description: ".xlsx を officework で開く"; Flags: unchecked
Name: "assocdocx"; Description: ".docx を officework で開く"; Flags: unchecked
; **うちの形は既定で入れる。** 横取りにならないので断る理由がない
Name: "assocadoc"; Description: ".adoc を officework で開く"

[Files]
Source: "{#Src}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; **配るのは officework 1本**(SEKKEI 段11)
Name: "{group}\officework";      Filename: "{app}\officework.exe"
Name: "{group}\はじめに";        Filename: "{app}\はじめに.md"
Name: "{group}\{cm:UninstallProgram,officework}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\officework"; Filename: "{app}\officework.exe"; Tasks: desktopicon

[Registry]
; **HKCU に書く**(管理者権限を求めないので HKLM は使えない)。
; 選ばれた関連付けだけ。消すときは一緒に消える
Root: HKCU; Subkey: "Software\Classes\officework.xlsx"; ValueType: string; ValueName: ""; ValueData: "Excel ブック"; Flags: uninsdeletekey; Tasks: assocxlsx
Root: HKCU; Subkey: "Software\Classes\officework.xlsx\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\officework.exe,0"; Tasks: assocxlsx
Root: HKCU; Subkey: "Software\Classes\officework.xlsx\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\officework.exe"" ""%1"""; Tasks: assocxlsx
Root: HKCU; Subkey: "Software\Classes\.xlsx"; ValueType: string; ValueName: ""; ValueData: "officework.xlsx"; Flags: uninsdeletevalue; Tasks: assocxlsx

Root: HKCU; Subkey: "Software\Classes\officework.docx"; ValueType: string; ValueName: ""; ValueData: "Word 文書"; Flags: uninsdeletekey; Tasks: assocdocx
Root: HKCU; Subkey: "Software\Classes\officework.docx\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\officework.exe,0"; Tasks: assocdocx
Root: HKCU; Subkey: "Software\Classes\officework.docx\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\officework.exe"" ""%1"""; Tasks: assocdocx
Root: HKCU; Subkey: "Software\Classes\.docx"; ValueType: string; ValueName: ""; ValueData: "officework.docx"; Flags: uninsdeletevalue; Tasks: assocdocx

; うちの形(.adoc)。**二重の拡張子(.sheet.adoc)も .adoc として届く** —
; Windows は最後の拡張子だけを見るので、これ1つで両方に効きます。
; 表か文章かは officework が名前を見て決めます
Root: HKCU; Subkey: "Software\Classes\officework.adoc"; ValueType: string; ValueName: ""; ValueData: "officework の文書"; Flags: uninsdeletekey; Tasks: assocadoc
Root: HKCU; Subkey: "Software\Classes\officework.adoc\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\officework.exe,0"; Tasks: assocadoc
Root: HKCU; Subkey: "Software\Classes\officework.adoc\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\officework.exe"" ""%1"""; Tasks: assocadoc
Root: HKCU; Subkey: "Software\Classes\.adoc"; ValueType: string; ValueName: ""; ValueData: "officework.adoc"; Flags: uninsdeletevalue; Tasks: assocadoc

[Run]
Filename: "{app}\officework.exe"; Description: "{cm:LaunchProgram,officework}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; 同梱 Python が入れた物(__pycache__ など)は [Files] の控えに無いので、
; 消し残さないように畳む。**利用者の ~/.config\officework には触らない** —
; マクロと設定は利用者の物
Type: filesandordirs; Name: "{app}\python"
