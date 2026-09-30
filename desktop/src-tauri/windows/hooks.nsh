; Per-user install. Tauri's default currentUser path is %LOCALAPPDATA%\Daylight.
; Force %LOCALAPPDATA%\Programs\Daylight so the shortcut and autostart agree.

!macro NSIS_HOOK_PREINSTALL
  StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Daylight"
  CreateDirectory "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Same HKCU Run value the Tauri autostart plugin writes (app name Daylight).
  MessageBox MB_YESNO|MB_ICONQUESTION "Start Daylight when you sign in to Windows?$\n$\nYou can change this later in Daylight settings." /SD IDNO IDNO skip_run
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Daylight" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  skip_run:
  IfFileExists "$SMPROGRAMS\Daylight.lnk" 0 aumid_done
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\Daylight.lnk"
  aumid_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Daylight"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
