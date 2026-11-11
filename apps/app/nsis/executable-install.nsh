!ifndef AXL_EXECUTABLE_INSTALL_INCLUDED
!define AXL_EXECUTABLE_INSTALL_INCLUDED

Var AxlExecutableStage
Var AxlInstallLock

!macro AxlExecutableMetadata SOURCE
  !tempfile AXL_EXECUTABLE_METADATA
  !system '"${AXL_INSTALLER_UI_PATH}" --describe-executable "${SOURCE}" --output "${AXL_EXECUTABLE_METADATA}"' = 0
  !include "${AXL_EXECUTABLE_METADATA}"
  !delfile "${AXL_EXECUTABLE_METADATA}"
  !undef AXL_EXECUTABLE_METADATA
!macroend

Function AxlLogExecutableInstall
  Exch $0
  Push $1
  ClearErrors
  FileOpen $1 "$INSTDIR\.axolotl-install.log" a
  ${IfNot} ${Errors}
    FileWrite $1 "$0$\r$\n"
    FileClose $1
  ${EndIf}
  Pop $1
  Pop $0
FunctionEnd

!macro AxlExecutableFailure MESSAGE
  Push "${MESSAGE}"
  Call AxlLogExecutableInstall
  ${If} $StatusFile != ""
    FileOpen $0 "$StatusFile" w
    FileWrite $0 "error:${MESSAGE}"
    FileClose $0
  ${EndIf}
  SetErrorLevel 2
  Abort "${MESSAGE}"
!macroend

!macro AxlAcquireInstallOwnership
  ; OPEN_ALWAYS never truncates the lock. The kernel releases the exclusive
  ; sharing handle after this installer and any replacement helper exit.
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\.axolotl-install.lock", i 0xC0000000, i 0, p 0, i 4, i 0x80, p 0) p.r0'
  ${If} $0 == -1
    !insertmacro AxlExecutableFailure "Another installer is using this directory, or it is not writable."
  ${EndIf}
  StrCpy $AxlInstallLock $0
!macroend

!macro AxlInstallExecutable SOURCE TARGET
  ClearErrors
  GetTempFileName $AxlExecutableStage "$INSTDIR"
  ${If} ${Errors}
    !insertmacro AxlExecutableFailure "Could not create a staged executable; the installed program was kept."
  ${EndIf}
  Push "Extracting executable to $AxlExecutableStage"
  Call AxlLogExecutableInstall
  ; Every extraction failure, including a killed installer, only damages this
  ; unique sibling file. Never point File at the installed main executable.
  File "/oname=$AxlExecutableStage" "${SOURCE}"
  InitPluginsDir
  File "/oname=$PLUGINSDIR\AxolotlExecutableInstall.exe" "${AXL_INSTALLER_UI_PATH}"
  StrCpy $0 2
  ; The helper duplicates this ownership handle before it examines the stage,
  ; so terminating the installer cannot admit a competing commit too early.
  System::Call 'kernel32::GetCurrentProcessId() i.r1'
  ClearErrors
  ExecWait '"$PLUGINSDIR\AxolotlExecutableInstall.exe" --install-executable "$AxlExecutableStage" "${TARGET}" "${AXL_EXECUTABLE_BYTES}" "${AXL_EXECUTABLE_SHA256}" "$1" "$AxlInstallLock"' $0
  ${If} ${Errors}
  ${OrIf} $0 != 0
    Delete "$AxlExecutableStage"
    !insertmacro AxlExecutableFailure "Could not verify or replace the executable. See .axolotl-install.log in the installation directory."
  ${EndIf}
!macroend

!endif
