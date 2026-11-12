; Hooks of the Windows installer (tauri.conf.json: bundle.windows.nsis.installerHooks).
;
; The app installs into a folder with an ASCII name ("Nisshoku"); what people
; see is localized: in Japanese, the shortcuts and the entry in Settings > Apps
; are named 日色 (日食 up to version 0.3.0: those shortcuts are renamed).

; Set when an install of version 0.1.0 (folder "日食") was replaced.
Var NisshokuMigrated

!macro NISSHOKU_NAME
  ; The shortcut name in the installer's language (chosen from Windows' display language).
  ${If} $LANGUAGE == 1041
    StrCpy $R9 "日色"
  ${Else}
    StrCpy $R9 "${PRODUCTNAME}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  StrCpy $NisshokuMigrated 0
  ; Version 0.1.0 installed into a folder named 日食. Remove that copy: /UPDATE
  ; keeps the notes and settings (they live in %APPDATA%\app.nisshoku.desktop).
  ${If} ${FileExists} "$LOCALAPPDATA\日食\uninstall.exe"
    ExecWait '"$LOCALAPPDATA\日食\uninstall.exe" /S /UPDATE _?=$LOCALAPPDATA\日食'
    Delete "$LOCALAPPDATA\日食\uninstall.exe"
    RMDir "$LOCALAPPDATA\日食"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\日食"
    StrCpy $NisshokuMigrated 1
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro NISSHOKU_NAME
  ; Shortcuts named 日食 (the app's name up to 0.3.0, and those of 0.1.0
  ; pointing into its removed folder) become ones to this install, under
  ; today's name. (An update does not make shortcuts by itself.)
  ${If} ${FileExists} "$SMPROGRAMS\日食.lnk"
  ${OrIf} $NisshokuMigrated = 1
    Delete "$SMPROGRAMS\日食.lnk"
    Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    CreateShortcut "$SMPROGRAMS\$R9.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$R9.lnk"
  ${EndIf}
  ${If} ${FileExists} "$DESKTOP\日食.lnk"
    Delete "$DESKTOP\日食.lnk"
    CreateShortcut "$DESKTOP\$R9.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\$R9.lnk"
  ${EndIf}
  ${If} $R9 != "${PRODUCTNAME}"
    ; Shortcuts made under the product name get the localized one.
    ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\$R9.lnk"
      Rename "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$SMPROGRAMS\$R9.lnk"
    ${EndIf}
    ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
      Delete "$DESKTOP\$R9.lnk"
      Rename "$DESKTOP\${PRODUCTNAME}.lnk" "$DESKTOP\$R9.lnk"
    ${EndIf}
    WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "$R9"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; The localized shortcuts (the uninstaller removes those named ${PRODUCTNAME}).
  ${If} $UpdateMode <> 1
    Delete "$SMPROGRAMS\日色.lnk"
    Delete "$DESKTOP\日色.lnk"
    Delete "$SMPROGRAMS\日食.lnk"
    Delete "$DESKTOP\日食.lnk"
  ${EndIf}
!macroend
