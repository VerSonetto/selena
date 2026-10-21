!macro NSIS_HOOK_POSTINSTALL
  ; The Tauri resource directory contains the console CLI. Put it next to the
  ; desktop executable so `selena desktop` can find selena-desktop.exe.
  ${If} ${FileExists} "$INSTDIR\bin\selena.exe"
    CopyFiles /SILENT "$INSTDIR\bin\selena.exe" "$INSTDIR\selena.exe"
  ${EndIf}

  ; Keep the path helper outside bin because Tauri removes bin before the
  ; uninstall hook runs.
  ${If} ${FileExists} "$INSTDIR\bin\path-hook.ps1"
    CopyFiles /SILENT "$INSTDIR\bin\path-hook.ps1" "$INSTDIR\selena-path.ps1"
    nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\selena-path.ps1" -Action add -Target "$INSTDIR"'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} ${FileExists} "$INSTDIR\selena-path.ps1"
    nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\selena-path.ps1" -Action remove -Target "$INSTDIR"'
    Delete "$INSTDIR\selena-path.ps1"
  ${EndIf}
  Delete /REBOOTOK "$INSTDIR\selena.exe"
!macroend
