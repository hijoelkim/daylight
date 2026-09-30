; Per-user install. Tauri's default currentUser path is %LOCALAPPDATA%\Daylight.
; Force %LOCALAPPDATA%\Programs\Daylight so the shortcut and autostart agree.
; SetOutPath must follow the INSTDIR change. Tauri calls SetOutPath before this
; hook, and the File commands keep that earlier directory. Shortcuts use $INSTDIR,
; so without this they point at an exe that was never copied there.

!macro NSIS_HOOK_PREINSTALL
  StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Daylight"
  CreateDirectory "$INSTDIR"
  SetOutPath $INSTDIR
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Same HKCU Run value the Tauri autostart plugin writes (app name Daylight).
  MessageBox MB_YESNO|MB_ICONQUESTION "Start Daylight when you sign in to Windows?$\n$\nYou can change this later in Daylight settings." /SD IDNO IDNO skip_run
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Daylight" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  skip_run:
  ; The cargo binary may be daylight.exe while the shortcut expects Daylight.exe.
  IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" shortcut_ok 0
    IfFileExists "$INSTDIR\daylight.exe" 0 shortcut_ok
      Rename "$INSTDIR\daylight.exe" "$INSTDIR\${MAINBINARYNAME}.exe"
  shortcut_ok:
  CreateShortcut "$SMPROGRAMS\Daylight.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\Daylight.lnk"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Daylight"
  Delete "$SMPROGRAMS\Daylight.lnk"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
