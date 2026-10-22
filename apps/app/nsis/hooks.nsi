; https://nsis.sourceforge.io/ShellExecWait
!ifndef AXL_INSTALLER_UI_PATH
  !if /FileExists "$%AXOLOTL_INSTALLER_UI_PATH%"
    !define AXL_INSTALLER_UI_PATH "$%AXOLOTL_INSTALLER_UI_PATH%"
  !else
    !define AXL_INSTALLER_UI_PATH "${__FILEDIR__}\..\..\..\target\release\axolotl-installer-ui.exe"
  !endif
!endif

!macro AxlCheckReparseRemoved PATH
    System::Call 'kernel32::GetFileAttributes(t "${PATH}") i.r5'
    ${If} $5 != -1
        SetErrorLevel 2
        Abort "Could not remove directory link: ${PATH}"
    ${EndIf}
!macroend

!macro ShellExecWait verb app param workdir show exitoutvar ;only app and show must be != "", every thing else is optional
    #define SEE_MASK_NOCLOSEPROCESS 0x40
    System::Store S
    !if "${NSIS_PTR_SIZE}" > 4
    !define /ReDef /math SYSSIZEOF_SHELLEXECUTEINFO 14 * ${NSIS_PTR_SIZE}
    !else ifndef SYSSIZEOF_SHELLEXECUTEINFO
    !define SYSSIZEOF_SHELLEXECUTEINFO 60
    !endif
    System::Call '*(&i${SYSSIZEOF_SHELLEXECUTEINFO})i.r0'
    System::Call '*$0(i ${SYSSIZEOF_SHELLEXECUTEINFO},i 0x40,p $hwndparent,t "${verb}",t $\'${app}$\',t $\'${param}$\',t "${workdir}",i ${show})p.r0'
    System::Call 'shell32::ShellExecuteEx(t)(pr0)i.r1 ?e' ; (t) to trigger A/W selection
    ${If} $1 <> 0
        System::Call '*$0(is,i,p,p,p,p,p,p,p,p,p,p,p,p,p.r1)' ;stack value not really used, just a fancy pop ;)
        System::Call 'kernel32::WaitForSingleObject(pr1,i-1)'
        System::Call 'kernel32::GetExitCodeProcess(pr1,*i.s)'
        System::Call 'kernel32::CloseHandle(pr1)'
    ${EndIf}
    System::Free $0
    !if "${exitoutvar}" == ""
        pop $0
    !endif
    System::Store L
    !if "${exitoutvar}" != ""
        pop ${exitoutvar}
    !endif
!macroend

; --------------------------------------------------------------------------------

Var /GLOBAL OldInstallDir

!macro NSIS_HOOK_PREINSTALL
    ; The updater is normally launched silently by the Tauri updater plugin.
    ; A current-user install can still target a protected directory, so elevate
    ; only the in-place update process instead of requiring UAC for first-time
    ; current-user installs.
    ${If} $UpdateMode == 1
      ${GetOptions} $CMDLINE "/ELEVATED" $0
      ${If} ${Errors}
        UserInfo::GetAccountType
        Pop $0
        ${If} $0 != "Admin"
          ExecShell "runas" "$EXEPATH" '$CMDLINE /ELEVATED'
          Abort
        ${EndIf}
      ${EndIf}
    ${EndIf}

    ; The updater already stops the running process and passes /UPDATE. Do not
    ; run the legacy uninstall/migration path during an in-place update, since
    ; it can resolve a different installation from the registry and relaunch
    ; the old executable after the new files are installed.
    ${If} $UpdateMode <> 1
      SetShellVarContext all
      ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
        UserInfo::GetAccountType
        Pop $0
        ${If} $0 != "Admin"
            MessageBox MB_ICONINFORMATION|MB_OK "An old installation of the Axolotl Launcher was detected that requires administrator permission to update from. You will be prompted with an admin prompt shortly."
        ${EndIf}

        ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
        ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"

        ReadRegStr $OldInstallDir SHCTX "${UNINSTKEY}" "InstallLocation"
        StrCpy $OldInstallDir $OldInstallDir "" 1
        StrCpy $OldInstallDir $OldInstallDir -1 ""

        DetailPrint "Executing $R1"
        !insertmacro ShellExecWait "runas" '$R1' '/P _?=$4' "" ${SW_SHOW} $3
        ${If} $3 <> 0
            SetErrorLevel $3
            MessageBox MB_ICONEXCLAMATION|MB_OK "Failed to uninstall old global installation"
            Abort
        ${EndIf}
      ${EndIf}
      SetShellVarContext current
    ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$OldInstallDir\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
        !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
        Return
    ${EndIf}
!macroend


!macro NSIS_HOOK_PREUNINSTALL
    ${If} $DeleteAppDataCheckboxState = 1
    ${AndIf} $UpdateMode <> 1
        ${If} ${FileExists} "$APPDATA\${BUNDLEID}"
            Push "$APPDATA\${BUNDLEID}"
            Call un.RemoveReparsePoints
        ${EndIf}
        ${If} ${FileExists} "$LOCALAPPDATA\${BUNDLEID}"
            Push "$LOCALAPPDATA\${BUNDLEID}"
            Call un.RemoveReparsePoints
        ${EndIf}
    ${EndIf}
!macroend

Function un.RemoveReparsePoints
    Exch $0 ; root directory to scan
    Push $1 ; FindFirst handle
    Push $2 ; current entry name
    Push $3 ; current entry path
    Push $4 ; entry attributes
    Push $5 ; spare

    ; A root junction must be removed before enumeration, too.
    System::Call 'kernel32::GetFileAttributes(t r0) i.r4'
    IntOp $5 $4 & 0x400
    ${If} $4 != -1
    ${AndIf} $5 != 0
        RmDir "$0"
        !insertmacro AxlCheckReparseRemoved "$0"
        Goto restore
    ${EndIf}

    FindFirst $1 $2 "$0\*"
    ${If} ${Errors}
        Goto done
    ${EndIf}

    loop:
        StrCmp $2 "." next
        StrCmp $2 ".." next

        StrCpy $3 "$0\$2"
        System::Call 'kernel32::GetFileAttributes(t r3) i.r4'

        IntOp $4 $4 & 0x400 ; FILE_ATTRIBUTE_REPARSE_POINT
        IntCmp $4 0 notReparse isReparse isReparse

        isReparse:
            System::Call 'kernel32::GetFileAttributes(t r3) i.r4'
            IntOp $4 $4 & 0x10 ; FILE_ATTRIBUTE_DIRECTORY
            IntCmp $4 0 removeFileLink removeDirLink removeDirLink

            removeDirLink:
                RmDir "$3" ; removes the junction / directory symlink itself
                !insertmacro AxlCheckReparseRemoved "$3"
                Goto next
            removeFileLink:
                Delete "$3" ; removes the file symlink itself
                !insertmacro AxlCheckReparseRemoved "$3"
                Goto next

        notReparse:
            System::Call 'kernel32::GetFileAttributes(t r3) i.r4'
            IntOp $4 $4 & 0x10
            IntCmp $4 0 next recurse recurse

            recurse:
                Push $3
                Call un.RemoveReparsePoints

        next:
            FindNext $1 $2
            IfErrors done
            Goto loop

    done:
        FindClose $1
    restore:
        Pop $5
        Pop $4
        Pop $3
        Pop $2
        Pop $1
        Pop $0
FunctionEnd
