Unicode true
ManifestDPIAware true
; Add in `dpiAwareness` `PerMonitorV2` to manifest for Windows 10 1607+ (note this should not affect lower versions since they should be able to ignore this and pick up `dpiAware` `true` set by `ManifestDPIAware true`)
; Currently undocumented on NSIS's website but is in the Docs folder of source tree, see
; https://github.com/kichik/nsis/blob/5fc0b87b819a9eec006df4967d08e522ddd651c9/Docs/src/attributes.but#L286-L300
; https://github.com/tauri-apps/tauri/pull/10106
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  ; Set the compression algorithm. We default to LZMA.
  SetCompressor /SOLID "{{compression}}"
!endif

; Keep above !include to stay ahead of any plugin command
; see https://github.com/tauri-apps/tauri/pull/15422#discussion_r3289239624
{{#if signed_plugins_path}}
!addplugindir "{{signed_plugins_path}}"
{{/if}}

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include "utils.nsh"
!include "FileAssociation.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

{{#if installer_hooks}}
!include "{{installer_hooks}}"
{{/if}}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!define LICENSE "{{license}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define UNINSTALLERICON "{{uninstaller_icon}}"
!define UNINSTALLERHEADERIMAGE "{{uninstaller_header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define WEBVIEW2INSTALLERARGS "{{webview2_installer_args}}"
!define WEBVIEW2BOOTSTRAPPERPATH "{{webview2_bootstrapper_path}}"
!define WEBVIEW2INSTALLERPATH "{{webview2_installer_path}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define STARTMENUFOLDER "{{start_menu_folder}}"

!define AXL_BACKGROUND 16181C
!define AXL_SURFACE 1D1F23
!define AXL_RAISED 27292E
!define AXL_CONTROL 34363C
!define AXL_BORDER 42444A
!define AXL_TEXT FFFFFF
!define AXL_TEXT_MUTED B0BAC5
!define AXL_TEXT_SECONDARY 96A2B0
!define AXL_BRAND F472B6
!define AXL_SUCCESS 1BD96A
!define AXL_ERROR FF496E

!define MUI_BGCOLOR ${AXL_BACKGROUND}
!define MUI_TEXTCOLOR ${AXL_TEXT}
!define MUI_INSTFILESPAGE_COLORS "${AXL_TEXT} ${AXL_BACKGROUND}"
!define MUI_WELCOMEPAGE_TITLE "$(axlWelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(axlWelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(axlFinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(axlFinishText)"
!define MUI_ABORTWARNING
!define MUI_CUSTOMFUNCTION_GUIINIT ApplyAxolotlTheme
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.ApplyAxolotlTheme

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
Var NoDesktopShortcutMode
Var WixMode
Var OldMainBinaryName
Var FreshInstall
Var ResourceDir
Var OptionsDialog
Var InstallDirInput
Var ResourceDirInput
Var ResourceDirLabel
Var ResourceDirDescription
Var ResourceDirBrowse
Var ResourceDirNotice
Var OptionsError
Var DesktopShortcutCheckbox
Var DesktopShortcutState
Var InstallerBodyFont
Var InstallerTitleFont
Var InstallerSmallFont
Var StatusFile
Var NoRunAfterInstall
Var UninstallUiChild
Var UninstallFailure

!macro AxlUninstallFile PATH
  Delete "${PATH}"
  Push "${PATH}"
  Call un.CheckPathRemoved
!macroend

!macro AxlStyleControl CONTROL FOREGROUND BACKGROUND
  SetCtlColors ${CONTROL} ${FOREGROUND} ${BACKGROUND}
  System::Call 'uxtheme::SetWindowTheme(p ${CONTROL}, w "DarkMode_Explorer", w "")'
!macroend

Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"

; We don't actually use this value as default install path,
; it's just for nsis to append the product name folder in the directory selector
; https://nsis.sourceforge.io/Reference/InstallDir
!define PLACEHOLDER_INSTALL_DIR "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

# additional plugins
!addplugindir "${ADDITIONALPLUGINSPATH}"

; Uninstaller signing command
!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

; Handle install mode, `perUser`, `perMachine` or `both`
!if "${INSTALLMODE}" == "perMachine"
  RequestExecutionLevel admin
!endif

!if "${INSTALLMODE}" == "currentUser"
  RequestExecutionLevel user
!endif

!if "${INSTALLMODE}" == "both"
  !define MULTIUSER_MUI
  !define MULTIUSER_INSTALLMODE_INSTDIR "${PRODUCTNAME}"
  !define MULTIUSER_INSTALLMODE_COMMANDLINE
  !if "${ARCH}" == "x64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !else if "${ARCH}" == "arm64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !endif
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${UNINSTKEY}"
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "CurrentUser"
  !define MULTIUSER_INSTALLMODEPAGE_SHOWUSERNAME
  !define MULTIUSER_INSTALLMODE_FUNCTION RestorePreviousInstallLocation
  !define MULTIUSER_EXECUTIONLEVEL Highest
  !include MultiUser.nsh
!endif

; Installer icon
!if "${INSTALLERICON}" != ""
  !define MUI_ICON "${INSTALLERICON}"
!endif

; Installer sidebar image
!if "${SIDEBARIMAGE}" != ""
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif

; Enable header images for installer and uninstaller pages when either image is configured.
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!else if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!endif

; Installer header image
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!endif

; Uninstaller header image
!if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"
!endif

; Uninstaller icon
!if "${UNINSTALLERICON}" != ""
  !define MUI_UNICON "${UNINSTALLERICON}"
!endif

; Define registry key to store installer language
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

; Installer pages, must be ordered as they appear
; 1. Welcome Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!define MUI_PAGE_CUSTOMFUNCTION_SHOW WelcomeShow
!insertmacro MUI_PAGE_WELCOME

; 2. License Page (if defined)
!if "${LICENSE}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!endif

; 3. Install mode (if it is set to `both`)
!if "${INSTALLMODE}" == "both"
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MULTIUSER_PAGE_INSTALLMODE
!endif

; 4. Custom page to ask user if he wants to reinstall/uninstall
;    only if a previous installation was detected
Var ReinstallPageCheck
Page custom PageReinstall PageLeaveReinstall
Function PageReinstall
  ; Uninstall previous WiX installation if exists.
  ;
  ; A WiX installer stores the installation info in registry
  ; using a UUID and so we have to loop through all keys under
  ; `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
  ; and check if `DisplayName` and `Publisher` keys match ${PRODUCTNAME} and ${MANUFACTURER}
  ;
  ; This has a potential issue that there maybe another installation that matches
  ; our ${PRODUCTNAME} and ${MANUFACTURER} but wasn't installed by our WiX installer,
  ; however, this should be fine since the user will have to confirm the uninstallation
  ; and they can chose to abort it if doesn't make sense.
  StrCpy $0 0
  wix_loop:
    EnumRegKey $1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" wix_loop_done ; Exit loop if there is no more keys to loop on
    IntOp $0 $0 + 1
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp "$R0$R1" "${PRODUCTNAME}${MANUFACTURER}" 0 wix_loop
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "UninstallString"
    ${StrCase} $R1 $R0 "L"
    ${StrLoc} $R0 $R1 "msiexec" ">"
    StrCmp $R0 0 0 wix_loop_done
    StrCpy $WixMode 1
    StrCpy $R6 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1"
    Goto compare_version
  wix_loop_done:

  ; Check if there is an existing installation, if not, abort the reinstall page
  ReadRegStr $R0 SHCTX "${UNINSTKEY}" ""
  ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
  ${IfThen} "$R0$R1" == "" ${|} Abort ${|}

  ; Compare this installar version with the existing installation
  ; and modify the messages presented to the user accordingly
  compare_version:
  StrCpy $R4 "$(older)"
  ${If} $WixMode = 1
    ReadRegStr $R0 HKLM "$R6" "DisplayVersion"
  ${Else}
    ReadRegStr $R0 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${EndIf}
  ${IfThen} $R0 == "" ${|} StrCpy $R4 "$(unknown)" ${|}

  nsis_tauri_utils::SemverCompare "${VERSION}" $R0
  Pop $R0
  ; Reinstalling the same version
  ${If} $R0 = 0
    StrCpy $R1 "$(alreadyInstalledLong)"
    StrCpy $R2 "$(addOrReinstall)"
    StrCpy $R3 "$(uninstallApp)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(chooseMaintenanceOption)"
  ; Upgrading
  ${ElseIf} $R0 = 1
    StrCpy $R1 "$(olderOrUnknownVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    StrCpy $R3 "$(dontUninstall)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
  ; Downgrading
  ${ElseIf} $R0 = -1
    StrCpy $R1 "$(newerVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    !if "${ALLOWDOWNGRADES}" == "true"
      StrCpy $R3 "$(dontUninstall)"
    !else
      StrCpy $R3 "$(dontUninstallDowngrade)"
    !endif
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
  ${Else}
    Abort
  ${EndIf}

  ; Skip showing the page if passive
  ;
  ; Note that we don't call this earlier at the beginning
  ; of this function because we need to populate some variables
  ; related to current installed version if detected and whether
  ; we are downgrading or not.
  ${If} $PassiveMode = 1
    Call PageLeaveReinstall
  ${Else}
    nsDialogs::Create 1018
    Pop $R4
    ${IfThen} $(^RTL) = 1 ${|} nsDialogs::SetRTL $(^RTL) ${|}
    SetCtlColors $R4 ${AXL_TEXT} ${AXL_BACKGROUND}

    ${NSD_CreateLabel} 0 0 100% 24u $R1
    Pop $R1
    SetCtlColors $R1 ${AXL_TEXT_MUTED} ${AXL_BACKGROUND}
    SendMessage $R1 ${WM_SETFONT} $InstallerBodyFont 1

    ${NSD_CreateRadioButton} 30u 50u -30u 8u $R2
    Pop $R2
    !insertmacro AxlStyleControl $R2 ${AXL_TEXT} ${AXL_BACKGROUND}
    ${NSD_OnClick} $R2 PageReinstallUpdateSelection

    ${NSD_CreateRadioButton} 30u 70u -30u 8u $R3
    Pop $R3
    !insertmacro AxlStyleControl $R3 ${AXL_TEXT} ${AXL_BACKGROUND}
    ; Disable this radio button if downgrading and downgrades are disabled
    !if "${ALLOWDOWNGRADES}" == "false"
      ${IfThen} $R0 = -1 ${|} EnableWindow $R3 0 ${|}
    !endif
    ${NSD_OnClick} $R3 PageReinstallUpdateSelection

    ; Check the first radio button if this the first time
    ; we enter this page or if the second button wasn't
    ; selected the last time we were on this page
    ${If} $ReinstallPageCheck <> 2
      SendMessage $R2 ${BM_SETCHECK} ${BST_CHECKED} 0
    ${Else}
      SendMessage $R3 ${BM_SETCHECK} ${BST_CHECKED} 0
    ${EndIf}

    ${NSD_SetFocus} $R2
    nsDialogs::Show
  ${EndIf}
FunctionEnd
Function PageReinstallUpdateSelection
  ${NSD_GetState} $R2 $R1
  ${If} $R1 == ${BST_CHECKED}
    StrCpy $ReinstallPageCheck 1
  ${Else}
    StrCpy $ReinstallPageCheck 2
  ${EndIf}
FunctionEnd
Function PageLeaveReinstall
  ${NSD_GetState} $R2 $R1

  ; If migrating from Wix, always uninstall
  ${If} $WixMode = 1
    Goto reinst_uninstall
  ${EndIf}

  ; In update mode, always proceeds without uninstalling
  ${If} $UpdateMode = 1
    Goto reinst_done
  ${EndIf}

  ; $R0 holds whether same(0)/upgrading(1)/downgrading(-1) version
  ; $R1 holds the radio buttons state:
  ;   1 => first choice was selected
  ;   0 => second choice was selected
  ${If} $R0 = 0 ; Same version, proceed
    ${If} $R1 = 1              ; User chose to add/reinstall
      Goto reinst_done
    ${Else}                    ; User chose to uninstall
      Goto reinst_uninstall
    ${EndIf}
  ${ElseIf} $R0 = 1 ; Upgrading
    ${If} $R1 = 1              ; User chose to uninstall
      Goto reinst_uninstall
    ${Else}
      Goto reinst_done         ; User chose NOT to uninstall
    ${EndIf}
  ${ElseIf} $R0 = -1 ; Downgrading
    ${If} $R1 = 1              ; User chose to uninstall
      Goto reinst_uninstall
    ${Else}
      Goto reinst_done         ; User chose NOT to uninstall
    ${EndIf}
  ${EndIf}

  reinst_uninstall:
    HideWindow
    ClearErrors

    ${If} $WixMode = 1
      ReadRegStr $R1 HKLM "$R6" "UninstallString"
      ExecWait '$R1' $0
    ${Else}
      ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
      ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
      ${IfThen} $UpdateMode = 1 ${|} StrCpy $R1 "$R1 /UPDATE" ${|} ; append /UPDATE
      ${IfThen} $PassiveMode = 1 ${|} StrCpy $R1 "$R1 /P" ${|} ; append /P
      StrCpy $R1 "$R1 _?=$4" ; append uninstall directory
      ExecWait '$R1' $0
    ${EndIf}

    BringToFront

    ${IfThen} ${Errors} ${|} StrCpy $0 2 ${|} ; ExecWait failed, set fake exit code

    ${If} $0 <> 0
    ${OrIf} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
      ; User cancelled wix uninstaller? return to select un/reinstall page
      ${If} $WixMode = 1
      ${AndIf} $0 = 1602
        Abort
      ${EndIf}

      ; User cancelled NSIS uninstaller? return to select un/reinstall page
      ${If} $0 = 1
        Abort
      ${EndIf}

      ; Other errors? show generic error message and return to select un/reinstall page
      MessageBox MB_ICONEXCLAMATION "$(unableToUninstall)"
      Abort
    ${EndIf}
  reinst_done:
FunctionEnd

Function ApplyAxolotlTheme
  CreateFont $InstallerBodyFont "Segoe UI" 9 400
  CreateFont $InstallerTitleFont "Segoe UI" 16 700
  CreateFont $InstallerSmallFont "Segoe UI" 8 400

  System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 20, *i 1, i 4)i.r0'
  ${If} $0 <> 0
    System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 19, *i 1, i 4)'
  ${EndIf}

  SetCtlColors $HWNDPARENT ${AXL_TEXT} ${AXL_BACKGROUND}
  GetDlgItem $0 $HWNDPARENT 1
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  GetDlgItem $0 $HWNDPARENT 2
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  GetDlgItem $0 $HWNDPARENT 3
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  GetDlgItem $0 $HWNDPARENT 1028
  !insertmacro AxlStyleControl $0 ${AXL_TEXT_SECONDARY} ${AXL_BACKGROUND}
FunctionEnd

Function StyleCurrentPage
  FindWindow $0 "#32770" "" $HWNDPARENT
  SetCtlColors $0 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $0 ${WM_SETFONT} $InstallerBodyFont 1

  GetDlgItem $1 $0 1200
  SetCtlColors $1 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $1 ${WM_SETFONT} $InstallerTitleFont 1
  GetDlgItem $1 $0 1201
  SetCtlColors $1 ${AXL_TEXT_MUTED} ${AXL_BACKGROUND}
  SendMessage $1 ${WM_SETFONT} $InstallerBodyFont 1
  GetDlgItem $1 $0 1004
  !insertmacro AxlStyleControl $1 ${AXL_TEXT_MUTED} ${AXL_SURFACE}
  GetDlgItem $1 $0 1027
  !insertmacro AxlStyleControl $1 ${AXL_TEXT} ${AXL_CONTROL}
FunctionEnd

Function WelcomeShow
  Call StyleCurrentPage
  GetDlgItem $0 $HWNDPARENT 1
  SendMessage $0 ${WM_SETTEXT} 0 "STR:$(axlStart)"
FunctionEnd

Function InstFilesShow
  Call StyleCurrentPage
FunctionEnd

Function FinishShow
  Call StyleCurrentPage
  GetDlgItem $0 $HWNDPARENT 1
  SendMessage $0 ${WM_SETTEXT} 0 "STR:$(axlFinishButton)"
FunctionEnd

Function TrimTrailingSlash
  Exch $0
  Push $1
  Push $2
  trim_trailing_slash_loop:
    StrLen $1 $0
    IntCmp $1 3 trim_trailing_slash_done trim_trailing_slash_done
    StrCpy $2 $0 1 -1
    StrCmp $2 "\" 0 trim_trailing_slash_done
    StrCpy $0 $0 -1
    Goto trim_trailing_slash_loop
  trim_trailing_slash_done:
  Pop $2
  Pop $1
  Exch $0
FunctionEnd

Function ReportInstallerProgress
  Exch $0
  Push $1
  ${If} $StatusFile != ""
    ClearErrors
    FileOpen $1 "$StatusFile" w
    ${IfNot} ${Errors}
      FileWrite $1 "$0"
      FileClose $1
    ${EndIf}
  ${EndIf}
  Pop $1
  Pop $0
FunctionEnd

Function UpdateResourceDirectoryNotice
  ${If} $FreshInstall <> 1
    Return
  ${EndIf}

  ${NSD_GetText} $ResourceDirInput $ResourceDir
  ${If} ${FileExists} "$ResourceDir\*.*"
    ${NSD_SetText} $ResourceDirNotice "$(axlExistingDirectoryNotice)"
    SetCtlColors $ResourceDirNotice ${AXL_TEXT_SECONDARY} ${AXL_BACKGROUND}
    ShowWindow $ResourceDirNotice ${SW_SHOW}
  ${Else}
    ShowWindow $ResourceDirNotice ${SW_HIDE}
  ${EndIf}
FunctionEnd

Function BrowseInstallDirectory
  ${NSD_GetText} $InstallDirInput $INSTDIR
  nsDialogs::SelectFolderDialog "$(axlSelectInstallDirectory)" "$INSTDIR"
  Pop $0
  ${If} $0 != error
    ${NSD_SetText} $InstallDirInput $0
  ${EndIf}
FunctionEnd

Function BrowseResourceDirectory
  ${NSD_GetText} $ResourceDirInput $ResourceDir
  nsDialogs::SelectFolderDialog "$(axlSelectResourceDirectory)" "$ResourceDir"
  Pop $0
  ${If} $0 != error
    ${NSD_SetText} $ResourceDirInput $0
    Call UpdateResourceDirectoryNotice
  ${EndIf}
FunctionEnd

Function PageOptions
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    Abort
  ${EndIf}

  nsDialogs::Create 1018
  Pop $OptionsDialog
  ${IfThen} $(^RTL) = 1 ${|} nsDialogs::SetRTL $(^RTL) ${|}
  SetCtlColors $OptionsDialog ${AXL_TEXT} ${AXL_BACKGROUND}

  ${NSD_CreateLabel} 0 0 100% 18u "$(axlOptionsTitle)"
  Pop $0
  SetCtlColors $0 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $0 ${WM_SETFONT} $InstallerTitleFont 1

  ${NSD_CreateLabel} 0 25u 100% 10u "$(axlInstallDirectoryLabel)"
  Pop $0
  SetCtlColors $0 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $0 ${WM_SETFONT} $InstallerBodyFont 1

  ${NSD_CreateText} 0 38u 77% 14u "$INSTDIR"
  Pop $InstallDirInput
  !insertmacro AxlStyleControl $InstallDirInput ${AXL_TEXT} ${AXL_CONTROL}
  SendMessage $InstallDirInput ${WM_SETFONT} $InstallerBodyFont 1

  ${NSD_CreateButton} 79% 38u 21% 14u "$(axlBrowse)"
  Pop $0
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  ${NSD_OnClick} $0 BrowseInstallDirectory

  ${If} $FreshInstall = 1
    ${NSD_CreateLabel} 0 61u 100% 10u "$(axlResourceDirectoryLabel)"
    Pop $ResourceDirLabel
    SetCtlColors $ResourceDirLabel ${AXL_TEXT} ${AXL_BACKGROUND}
    SendMessage $ResourceDirLabel ${WM_SETFONT} $InstallerBodyFont 1

    ${NSD_CreateLabel} 0 73u 100% 18u "$(axlResourceDirectoryDescription)"
    Pop $ResourceDirDescription
    SetCtlColors $ResourceDirDescription ${AXL_TEXT_MUTED} ${AXL_BACKGROUND}
    SendMessage $ResourceDirDescription ${WM_SETFONT} $InstallerSmallFont 1

    ${NSD_CreateText} 0 94u 77% 14u "$ResourceDir"
    Pop $ResourceDirInput
    !insertmacro AxlStyleControl $ResourceDirInput ${AXL_TEXT} ${AXL_CONTROL}
    SendMessage $ResourceDirInput ${WM_SETFONT} $InstallerBodyFont 1

    ${NSD_CreateButton} 79% 94u 21% 14u "$(axlBrowse)"
    Pop $ResourceDirBrowse
    !insertmacro AxlStyleControl $ResourceDirBrowse ${AXL_TEXT} ${AXL_CONTROL}
    ${NSD_OnClick} $ResourceDirBrowse BrowseResourceDirectory

    ${NSD_CreateLabel} 0 111u 100% 18u ""
    Pop $ResourceDirNotice
    SendMessage $ResourceDirNotice ${WM_SETFONT} $InstallerSmallFont 1
    ShowWindow $ResourceDirNotice ${SW_HIDE}
    Call UpdateResourceDirectoryNotice
  ${Else}
    ${NSD_CreateLabel} 0 64u 100% 34u "$(axlExistingInstallDirectoryPreserved)"
    Pop $0
    SetCtlColors $0 ${AXL_TEXT_MUTED} ${AXL_SURFACE}
    SendMessage $0 ${WM_SETFONT} $InstallerBodyFont 1
  ${EndIf}

  ${NSD_CreateCheckbox} 0 134u 100% 12u "$(createDesktop)"
  Pop $DesktopShortcutCheckbox
  !insertmacro AxlStyleControl $DesktopShortcutCheckbox ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $DesktopShortcutCheckbox ${WM_SETFONT} $InstallerBodyFont 1
  ${NSD_SetState} $DesktopShortcutCheckbox $DesktopShortcutState

  ${NSD_CreateLabel} 0 151u 100% 20u ""
  Pop $OptionsError
  SetCtlColors $OptionsError ${AXL_ERROR} ${AXL_BACKGROUND}
  SendMessage $OptionsError ${WM_SETFONT} $InstallerSmallFont 1

  GetDlgItem $0 $HWNDPARENT 1
  SendMessage $0 ${WM_SETTEXT} 0 "STR:$(axlInstallButton)"
  GetDlgItem $0 $HWNDPARENT 3
  SendMessage $0 ${WM_SETTEXT} 0 "STR:$(axlBack)"
  ${NSD_SetFocus} $InstallDirInput
  nsDialogs::Show
FunctionEnd

Function PageLeaveOptions
  ${NSD_GetText} $InstallDirInput $INSTDIR
  Push $INSTDIR
  Call TrimTrailingSlash
  Pop $INSTDIR
  ${GetRoot} "$INSTDIR" $0
  ${If} $0 == ""
    ${NSD_SetText} $OptionsError "$(axlInstallDirectoryInvalid)"
    Abort
  ${EndIf}

  ${NSD_GetState} $DesktopShortcutCheckbox $DesktopShortcutState

  ${If} $FreshInstall <> 1
    Return
  ${EndIf}

  ${NSD_GetText} $ResourceDirInput $ResourceDir
  Push $ResourceDir
  Call TrimTrailingSlash
  Pop $ResourceDir
  ${NSD_SetText} $ResourceDirInput $ResourceDir

  ${GetRoot} "$ResourceDir" $0
  ${If} $0 == ""
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryAbsolute)"
    Abort
  ${EndIf}
  ${If} "$ResourceDir" == "$0"
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryRoot)"
    Abort
  ${EndIf}
  ${If} "$ResourceDir" == "$0\"
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryRoot)"
    Abort
  ${EndIf}

  ${StrCase} $0 "$INSTDIR" "L"
  ${StrCase} $1 "$ResourceDir" "L"
  ${If} $0 == $1
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryInsideInstall)"
    Abort
  ${EndIf}
  StrCpy $2 "$0\"
  StrLen $3 $2
  StrCpy $4 $1 $3
  ${If} $4 == $2
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryInsideInstall)"
    Abort
  ${EndIf}

  ClearErrors
  CreateDirectory "$ResourceDir"
  FileOpen $0 "$ResourceDir\.axolotl-write-test" w
  ${If} ${Errors}
    ClearErrors
    ${NSD_SetText} $OptionsError "$(axlResourceDirectoryNotWritable)"
    Abort
  ${EndIf}
  FileWrite $0 "Axolotl Launcher"
  FileClose $0
  Delete "$ResourceDir\.axolotl-write-test"
FunctionEnd

; 5. Choose program and application directories
Page custom PageOptions PageLeaveOptions

; 6. Start menu shortcut page
Var AppStartMenuFolder
!if "${STARTMENUFOLDER}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !define MUI_STARTMENUPAGE_DEFAULTFOLDER "${STARTMENUFOLDER}"
!else
  !define MUI_PAGE_CUSTOMFUNCTION_PRE Skip
!endif
!insertmacro MUI_PAGE_STARTMENU Application $AppStartMenuFolder

; 7. Installation page
!define MUI_PAGE_CUSTOMFUNCTION_SHOW InstFilesShow
!insertmacro MUI_PAGE_INSTFILES

; 8. Finish page
;
; Don't auto jump to finish page after installation page,
; because the installation page has useful info that can be used debug any issues with the installer.
!define MUI_FINISHPAGE_NOAUTOCLOSE
; Show run app after installation.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!define MUI_PAGE_CUSTOMFUNCTION_SHOW FinishShow
!insertmacro MUI_PAGE_FINISH

Function RunMainBinary
  nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
FunctionEnd

; Uninstaller Pages
; 1. Confirm uninstall page
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
!define /ifndef WS_EX_LAYOUTRTL         0x00400000
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
Function un.ConfirmShow ; Add add a `Delete app data` check box
  ; $1 inner dialog HWND
  ; $2 window DPI
  ; $3 style
  ; $4 x
  ; $5 y
  ; $6 width
  ; $7 height
  FindWindow $1 "#32770" "" $HWNDPARENT ; Find inner dialog
  System::Call "user32::GetDpiForWindow(p r1) i .r2"
  ${If} $(^RTL) = 1
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE} | ${WS_EX_LAYOUTRTL}"
    IntOp $4 50 * $2
  ${Else}
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE}"
    IntOp $4 0 * $2
  ${EndIf}
  IntOp $5 100 * $2
  IntOp $6 400 * $2
  IntOp $7 25 * $2
  IntOp $4 $4 / 96
  IntOp $5 $5 / 96
  IntOp $6 $6 / 96
  IntOp $7 $7 / 96
  System::Call 'user32::CreateWindowEx(i r3, w "${__NSD_CheckBox_CLASS}", w "$(deleteAppData)", i ${__NSD_CheckBox_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
  Pop $DeleteAppDataCheckbox
  SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
  SetCtlColors $HWNDPARENT ${AXL_TEXT} ${AXL_BACKGROUND}
  FindWindow $1 "#32770" "" $HWNDPARENT
  SetCtlColors $1 ${AXL_TEXT} ${AXL_BACKGROUND}
  SetCtlColors $DeleteAppDataCheckbox ${AXL_TEXT} ${AXL_BACKGROUND}
  System::Call 'uxtheme::SetWindowTheme(p $DeleteAppDataCheckbox, w "DarkMode_Explorer", w "")'
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
Function un.ConfirmLeave
  SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM

; 2. Uninstalling Page
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.InstFilesShow
!insertmacro MUI_UNPAGE_INSTFILES

; 3. Finished Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.FinishShow
!insertmacro MUI_UNPAGE_FINISH

Function un.ApplyAxolotlTheme
  CreateFont $InstallerBodyFont "Segoe UI" 9 400
  CreateFont $InstallerTitleFont "Segoe UI" 16 700
  CreateFont $InstallerSmallFont "Segoe UI" 8 400
  System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 20, *i 1, i 4)i.r0'
  ${If} $0 <> 0
    System::Call 'dwmapi::DwmSetWindowAttribute(p $HWNDPARENT, i 19, *i 1, i 4)'
  ${EndIf}
  SetCtlColors $HWNDPARENT ${AXL_TEXT} ${AXL_BACKGROUND}
  GetDlgItem $0 $HWNDPARENT 1
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  GetDlgItem $0 $HWNDPARENT 2
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
  GetDlgItem $0 $HWNDPARENT 3
  !insertmacro AxlStyleControl $0 ${AXL_TEXT} ${AXL_CONTROL}
FunctionEnd

Function un.StyleCurrentPage
  FindWindow $0 "#32770" "" $HWNDPARENT
  SetCtlColors $0 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $0 ${WM_SETFONT} $InstallerBodyFont 1
  GetDlgItem $1 $0 1200
  SetCtlColors $1 ${AXL_TEXT} ${AXL_BACKGROUND}
  SendMessage $1 ${WM_SETFONT} $InstallerTitleFont 1
  GetDlgItem $1 $0 1201
  SetCtlColors $1 ${AXL_TEXT_MUTED} ${AXL_BACKGROUND}
  SendMessage $1 ${WM_SETFONT} $InstallerBodyFont 1
  GetDlgItem $1 $0 1004
  !insertmacro AxlStyleControl $1 ${AXL_TEXT_MUTED} ${AXL_SURFACE}
  GetDlgItem $1 $0 1027
  !insertmacro AxlStyleControl $1 ${AXL_TEXT} ${AXL_CONTROL}
FunctionEnd

Function un.InstFilesShow
  Call un.StyleCurrentPage
FunctionEnd

Function un.FinishShow
  Call un.StyleCurrentPage
FunctionEnd

;Languages
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
  !include "{{this}}"
{{/each}}

LangString axlWelcomeTitle ${LANG_ENGLISH} "Install Axolotl Launcher"
LangString axlWelcomeText ${LANG_ENGLISH} "A modern home for your Minecraft instances.$\r$\n$\r$\nVersion ${VERSION} · 64-bit · Current user"
LangString axlStart ${LANG_ENGLISH} "Start"
LangString axlOptionsTitle ${LANG_ENGLISH} "Installation settings"
LangString axlInstallDirectoryLabel ${LANG_ENGLISH} "Program installation location"
LangString axlResourceDirectoryLabel ${LANG_ENGLISH} "Application directory"
LangString axlResourceDirectoryDescription ${LANG_ENGLISH} "Stores game instances, Java runtimes, resources, and caches. This directory can grow significantly."
LangString axlBrowse ${LANG_ENGLISH} "Browse..."
LangString axlSelectInstallDirectory ${LANG_ENGLISH} "Select the program installation location"
LangString axlSelectResourceDirectory ${LANG_ENGLISH} "Select the application directory"
LangString axlExistingDirectoryNotice ${LANG_ENGLISH} "This directory already contains files. Axolotl will keep them."
LangString axlExistingInstallDirectoryPreserved ${LANG_ENGLISH} "Your existing application directory will be preserved. You can change it later in Settings > Resource management."
LangString axlInstallButton ${LANG_ENGLISH} "Install"
LangString axlBack ${LANG_ENGLISH} "Back"
LangString axlInstallDirectoryInvalid ${LANG_ENGLISH} "Choose an absolute program installation location."
LangString axlResourceDirectoryAbsolute ${LANG_ENGLISH} "Choose an absolute application directory."
LangString axlResourceDirectoryRoot ${LANG_ENGLISH} "The root of a drive or network share cannot be used as the application directory."
LangString axlResourceDirectoryInsideInstall ${LANG_ENGLISH} "The application directory must be separate from the program installation location."
LangString axlResourceDirectoryNotWritable ${LANG_ENGLISH} "Axolotl cannot write to this application directory. Choose another location or change its permissions."
LangString axlFinishTitle ${LANG_ENGLISH} "Axolotl Launcher is installed"
LangString axlFinishText ${LANG_ENGLISH} "Everything is ready. Your application directory is:$\r$\n$ResourceDir"
LangString axlFinishButton ${LANG_ENGLISH} "Finish"

LangString axlWelcomeTitle ${LANG_SIMPCHINESE} "安装 Axolotl Launcher"
LangString axlWelcomeText ${LANG_SIMPCHINESE} "为你的 Minecraft 实例准备的现代化启动器。$\r$\n$\r$\n版本 ${VERSION} · 64 位 · 当前用户"
LangString axlStart ${LANG_SIMPCHINESE} "开始"
LangString axlOptionsTitle ${LANG_SIMPCHINESE} "安装设置"
LangString axlInstallDirectoryLabel ${LANG_SIMPCHINESE} "程序安装位置"
LangString axlResourceDirectoryLabel ${LANG_SIMPCHINESE} "应用目录"
LangString axlResourceDirectoryDescription ${LANG_SIMPCHINESE} "用于保存游戏实例、Java、游戏资源和缓存，后续可能占用较多空间。"
LangString axlBrowse ${LANG_SIMPCHINESE} "浏览..."
LangString axlSelectInstallDirectory ${LANG_SIMPCHINESE} "选择程序安装位置"
LangString axlSelectResourceDirectory ${LANG_SIMPCHINESE} "选择应用目录"
LangString axlExistingDirectoryNotice ${LANG_SIMPCHINESE} "此目录中已有文件，Axolotl 会保留这些内容。"
LangString axlExistingInstallDirectoryPreserved ${LANG_SIMPCHINESE} "将保留现有应用目录。之后仍可在“设置 > 资源管理”中修改。"
LangString axlInstallButton ${LANG_SIMPCHINESE} "安装"
LangString axlBack ${LANG_SIMPCHINESE} "上一步"
LangString axlInstallDirectoryInvalid ${LANG_SIMPCHINESE} "请选择有效的绝对程序安装路径。"
LangString axlResourceDirectoryAbsolute ${LANG_SIMPCHINESE} "请选择有效的绝对应用目录。"
LangString axlResourceDirectoryRoot ${LANG_SIMPCHINESE} "不能将磁盘或网络共享根目录直接用作应用目录。"
LangString axlResourceDirectoryInsideInstall ${LANG_SIMPCHINESE} "应用目录必须与程序安装位置分开。"
LangString axlResourceDirectoryNotWritable ${LANG_SIMPCHINESE} "Axolotl 无法写入该应用目录，请更换位置或调整目录权限。"
LangString axlFinishTitle ${LANG_SIMPCHINESE} "Axolotl Launcher 已安装"
LangString axlFinishText ${LANG_SIMPCHINESE} "一切准备就绪。当前应用目录：$\r$\n$ResourceDir"
LangString axlFinishButton ${LANG_SIMPCHINESE} "完成"

Function RequireInstallerWebView2
  Push $0
  Push $1
  ; Evergreen registers in the 32-bit view for both machine and user installs.
  SetRegView 32
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${VersionCompare} "$0" "0.0.0.0" $1
  ${If} $0 == ""
  ${OrIf} $1 <> 1
    ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
    ${VersionCompare} "$0" "0.0.0.0" $1
  ${EndIf}
  SetRegView lastused
  ${If} $0 != ""
  ${AndIf} $1 = 1
    Pop $1
    Pop $0
    Return
  ${EndIf}
  Pop $1
  Pop $0
  Call ShowMissingInstallerWebView2
FunctionEnd

Function ShowMissingInstallerWebView2
  InitPluginsDir
  File "/oname=$PLUGINSDIR\AxolotlInstallerUI.exe" "${AXL_INSTALLER_UI_PATH}"
  StrCpy $0 2
  ExecWait '"$PLUGINSDIR\AxolotlInstallerUI.exe" --webview2-required --language "$LANGUAGE"' $0
  ${If} $0 = 10
    ExecShell "open" "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
  ${EndIf}
  SetErrorLevel 2
  Quit
FunctionEnd

Function .onInit
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NS" $NoShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NO_DESKTOP_SHORTCUT" $NoDesktopShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoDesktopShortcutMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NO_RUN_AFTER_INSTALL" $0
  ${IfNot} ${Errors}
    StrCpy $NoRunAfterInstall 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/STATUS_FILE=" $StatusFile
  ${If} ${Errors}
    StrCpy $StatusFile ""
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  !if "${DISPLAYLANGUAGESELECTOR}" == "true"
    !insertmacro MUI_LANGDLL_DISPLAY
  !endif

  !insertmacro SetContext

  StrCpy $ResourceDir "$APPDATA\${BUNDLEID}"
  ${GetOptions} $CMDLINE "/RESOURCE_DIR=" $0
  ${IfNot} ${Errors}
    StrCpy $ResourceDir "$0"
  ${EndIf}
  StrCpy $DesktopShortcutState ${BST_CHECKED}
  StrCpy $FreshInstall 1
  ReadRegStr $0 SHCTX "${UNINSTKEY}" "UninstallString"
  ${If} $0 != ""
    StrCpy $FreshInstall 0
  ${EndIf}
  ${If} ${FileExists} "$APPDATA\${BUNDLEID}\app.db"
    StrCpy $FreshInstall 0
  ${EndIf}
  ${If} $UpdateMode = 1
    StrCpy $FreshInstall 0
  ${EndIf}

  ${If} $INSTDIR == "${PLACEHOLDER_INSTALL_DIR}"
    ; Set default install location
    !if "${INSTALLMODE}" == "perMachine"
      ${If} ${RunningX64}
        !if "${ARCH}" == "x64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else if "${ARCH}" == "arm64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else
          StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
        !endif
      ${Else}
        StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
      ${EndIf}
    !else if "${INSTALLMODE}" == "currentUser"
      StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
    !endif

    Call RestorePreviousInstallLocation
  ${EndIf}

  ${GetOptions} $CMDLINE "/INSTALL_DIR=" $0
  ${IfNot} ${Errors}
    StrCpy $INSTDIR "$0"
  ${EndIf}


  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_INIT
  !endif

  Push 4
  Call ReportInstallerProgress

  ${IfNot} ${Silent}
  ${AndIf} $PassiveMode <> 1
    Call RequireInstallerWebView2
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File "/oname=$PLUGINSDIR\AxolotlInstallerUI.exe" "${AXL_INSTALLER_UI_PATH}"
    StrCpy $0 2
    ClearErrors
    ExecWait '"$PLUGINSDIR\AxolotlInstallerUI.exe" --installer "$EXEPATH" --version "${VERSION}" --install-dir "$INSTDIR" --resource-dir "$ResourceDir" --fresh-install "$FreshInstall" --language "$LANGUAGE"' $0
    ${If} $0 <> 2
      Quit
    ${EndIf}
  ${EndIf}
FunctionEnd


Section EarlyChecks
  Push 8
  Call ReportInstallerProgress
  ; Abort silent installer if downgrades is disabled
  !if "${ALLOWDOWNGRADES}" == "false"
  ${If} ${Silent}
    ; If downgrading
    ${If} $R0 = -1
      System::Call 'kernel32::AttachConsole(i -1)i.r0'
      ${If} $0 <> 0
        System::Call 'kernel32::GetStdHandle(i -11)i.r0'
        System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)' ; set red color
        FileWrite $0 "$(silentDowngrades)"
      ${EndIf}
      Abort
    ${EndIf}
  ${EndIf}
  !endif

SectionEnd

Section WebView2
  Push 12
  Call ReportInstallerProgress
  ; Check if Webview2 is already installed and skip this section
  ${If} ${RunningX64}
    ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${Else}
    ReadRegStr $4 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ${If} $4 == ""
    ReadRegStr $4 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}

  ${If} $4 == ""
    ; Webview2 installation
    ;
    ; Skip if updating
    ${If} $UpdateMode <> 1
      !if "${INSTALLWEBVIEW2MODE}" == "downloadBootstrapper"
        Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
        DetailPrint "$(webview2Downloading)"
        NSISdl::download "https://go.microsoft.com/fwlink/p/?LinkId=2124703" "$TEMP\MicrosoftEdgeWebview2Setup.exe"
        Pop $0
        ${If} $0 == "success"
          DetailPrint "$(webview2DownloadSuccess)"
        ${Else}
          DetailPrint "$(webview2DownloadError)"
          Abort "$(webview2AbortError)"
        ${EndIf}
        StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
        Goto install_webview2
      !endif

      !if "${INSTALLWEBVIEW2MODE}" == "embedBootstrapper"
        Delete "$TEMP\MicrosoftEdgeWebview2Setup.exe"
        File "/oname=$TEMP\MicrosoftEdgeWebview2Setup.exe" "${WEBVIEW2BOOTSTRAPPERPATH}"
        DetailPrint "$(installingWebview2)"
        StrCpy $6 "$TEMP\MicrosoftEdgeWebview2Setup.exe"
        Goto install_webview2
      !endif

      !if "${INSTALLWEBVIEW2MODE}" == "offlineInstaller"
        Delete "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
        File "/oname=$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe" "${WEBVIEW2INSTALLERPATH}"
        DetailPrint "$(installingWebview2)"
        StrCpy $6 "$TEMP\MicrosoftEdgeWebView2RuntimeInstaller.exe"
        Goto install_webview2
      !endif

      Goto webview2_done

      install_webview2:
        DetailPrint "$(installingWebview2)"
        ; $6 holds the path to the webview2 installer
        ExecWait "$6 ${WEBVIEW2INSTALLERARGS} /install" $1
        ${If} $1 = 0
          DetailPrint "$(webview2InstallSuccess)"
        ${Else}
          DetailPrint "$(webview2InstallError)"
          Abort "$(webview2AbortError)"
        ${EndIf}
      webview2_done:
    ${EndIf}
  ${Else}
    !if "${MINIMUMWEBVIEW2VERSION}" != ""
      ${VersionCompare} "${MINIMUMWEBVIEW2VERSION}" "$4" $R0
      ${If} $R0 = 1
        update_webview:
          DetailPrint "$(installingWebview2)"
          ${If} ${RunningX64}
            ReadRegStr $R1 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate" "path"
          ${Else}
            ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\EdgeUpdate" "path"
          ${EndIf}
          ${If} $R1 == ""
            ReadRegStr $R1 HKCU "SOFTWARE\Microsoft\EdgeUpdate" "path"
          ${EndIf}
          ${If} $R1 != ""
            ; Chromium updater docs: https://source.chromium.org/chromium/chromium/src/+/main:docs/updater/user_manual.md
            ; Modified from "HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Microsoft EdgeWebView\ModifyPath"
            ExecWait `"$R1" /install appguid=${WEBVIEW2APPGUID}&needsadmin=true` $1
            ${If} $1 = 0
              DetailPrint "$(webview2InstallSuccess)"
            ${Else}
              MessageBox MB_ICONEXCLAMATION|MB_ABORTRETRYIGNORE "$(webview2InstallError)" IDIGNORE ignore IDRETRY update_webview
              Quit
              ignore:
            ${EndIf}
          ${EndIf}
      ${EndIf}
    !endif
  ${EndIf}
  Push 22
  Call ReportInstallerProgress
SectionEnd

Section Install
  Push 30
  Call ReportInstallerProgress
  SetOutPath $INSTDIR

  !ifmacrodef NSIS_HOOK_PREINSTALL
    !insertmacro NSIS_HOOK_PREINSTALL
  !endif

  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  ; Copy main executable
  File "${MAINBINARYSRCPATH}"

  ; Copy resources
  {{#each resources_dirs}}
    CreateDirectory "$INSTDIR\\{{this}}"
  {{/each}}
  {{#each resources}}
    File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
  {{/each}}

  ; Copy external binaries
  {{#each binaries}}
    File /a "/oname={{this}}" "{{no-escape @key}}"
  {{/each}}

  Push 64
  Call ReportInstallerProgress

  ; Create file associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
       !insertmacro APP_ASSOCIATE "{{ext}}" "{{or association.name ext}}" "{{association-description association.description ext}}" "$INSTDIR\${MAINBINARYNAME}.exe,0" "Open with ${PRODUCTNAME}" "$INSTDIR\${MAINBINARYNAME}.exe $\"%1$\""
    {{/each}}
  {{/each}}

  ; Register deep links
  {{#each deep_link_protocols as |protocol| ~}}
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "URL Protocol" ""
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "" "URL:${BUNDLEID} protocol"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  {{/each}}

  ; Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Save $INSTDIR in registry for future installations
  WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR

  !if "${INSTALLMODE}" == "both"
    ; Save install mode to be selected by default for the next installation such as updating
    ; or when uninstalling
    WriteRegStr SHCTX "${UNINSTKEY}" $MultiUser.InstallMode 1
  !endif

  ; Remove old main binary if it doesn't match new main binary name
  ReadRegStr $OldMainBinaryName SHCTX "${UNINSTKEY}" "MainBinaryName"
  ${If} $OldMainBinaryName != ""
  ${AndIf} $OldMainBinaryName != "${MAINBINARYNAME}.exe"
    Delete "$INSTDIR\$OldMainBinaryName"
  ${EndIf}

  ; Save current MAINBINARYNAME for future updates
  WriteRegStr SHCTX "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"

  ; Registry information for add/remove programs
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"

  ${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
  IntOp $0 $0 + ${ESTIMATEDSIZE}
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "$0"

  !if "${HOMEPAGE}" != ""
    WriteRegStr SHCTX "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
    WriteRegStr SHCTX "${UNINSTKEY}" "URLUpdateInfo" "${HOMEPAGE}"
    WriteRegStr SHCTX "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
  !endif

  ; Create start menu shortcut
  !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
    Call CreateOrUpdateStartMenuShortcut
  !insertmacro MUI_STARTMENU_WRITE_END

  Push 82
  Call ReportInstallerProgress

  ; Create desktop shortcut for silent and passive installers
  ; because finish page will be skipped
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    Call CreateOrUpdateDesktopShortcut
  ${ElseIf} $DesktopShortcutState = ${BST_CHECKED}
    Call CreateOrUpdateDesktopShortcut
  ${EndIf}

  !ifmacrodef NSIS_HOOK_POSTINSTALL
    !insertmacro NSIS_HOOK_POSTINSTALL
  !endif

  Push 96
  Call ReportInstallerProgress

  ; Hand the first-install application directory to the launcher. The launcher
  ; consumes this value after creating its database and before initializing
  ; the existing directory migration pipeline.
  ${If} $FreshInstall = 1
    ${StrCase} $0 "$ResourceDir" "L"
    ${StrCase} $1 "$APPDATA\${BUNDLEID}" "L"
    ${If} $0 == $1
      DeleteRegValue HKCU "${MANUPRODUCTKEY}" "PendingResourceDirectory"
    ${Else}
      WriteRegStr HKCU "${MANUPRODUCTKEY}" "PendingResourceDirectory" "$ResourceDir"
    ${EndIf}
  ${EndIf}

  ; Auto close this page for passive mode
  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Function .onInstSuccess
  Push 100
  Call ReportInstallerProgress

  ; Installer-ui runs the silent core and starts the app itself based on the
  ; user's "Launch when complete" choice, so never auto-launch there.
  ${If} $NoRunAfterInstall <> 1
    ; Check for `/R` flag only in silent and passive installers because
    ; GUI installer has a toggle for the user to (re)start the app
    ${If} $PassiveMode = 1
    ${OrIf} ${Silent}
      ${GetOptions} $CMDLINE "/R" $R0
      ${IfNot} ${Errors}
        ${GetOptions} $CMDLINE "/ARGS" $R0
        nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.onInit
  !insertmacro SetContext

  StrCpy $UninstallFailure ""
  ${GetOptions} $CMDLINE "/STATUS_FILE=" $StatusFile
  ${If} ${Errors}
    StrCpy $StatusFile ""
  ${EndIf}

  StrCpy $UninstallUiChild 0
  ${GetOptions} $CMDLINE "/UI_CHILD" $0
  ${IfNot} ${Errors}
    StrCpy $UninstallUiChild 1
  ${EndIf}

  StrCpy $DeleteAppDataCheckboxState 0
  ${GetOptions} $CMDLINE "/DELETE_APP_DATA" $0
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState 1
  ${EndIf}

  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_UNINIT
  !endif

  !insertmacro MUI_UNGETLANGUAGE

  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  ${If} $UninstallUiChild <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIf} $UpdateMode <> 1
  ${AndIfNot} ${Silent}
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File "/oname=$PLUGINSDIR\AxolotlInstallerUI.exe" "${AXL_INSTALLER_UI_PATH}"
    ExecWait '"$PLUGINSDIR\AxolotlInstallerUI.exe" --uninstaller "$EXEPATH" --install-dir "$INSTDIR" --version "${VERSION}" --language "$LANGUAGE"' $0
    SetErrorLevel $0
    Quit
  ${EndIf}
FunctionEnd

Function un.ReportInstallerProgress
  Exch $0
  Push $1
  ${If} $StatusFile != ""
    ClearErrors
    FileOpen $1 "$StatusFile" w
    ${IfNot} ${Errors}
      FileWriteUTF16LE /BOM $1 "$0"
      FileClose $1
    ${EndIf}
  ${EndIf}
  Pop $1
  Pop $0
FunctionEnd

Function un.CheckPathRemoved
  Exch $0
  Push $1
  Push $2
  System::Call 'kernel32::GetFileAttributes(t r0)i.r1 ?e'
  Pop $2
  ${If} $1 != -1
    StrCpy $UninstallFailure "Could not remove: $0"
  ${ElseIf} $2 != 2
  ${AndIf} $2 != 3
    StrCpy $UninstallFailure "Could not verify removal: $0 (Windows error $2)"
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
  ${If} $UninstallFailure != ""
    SetErrorLevel 2
    Abort
  ${EndIf}
FunctionEnd

Function un.onUninstFailed
  ${If} $UninstallFailure == ""
    StrCpy $UninstallFailure "Uninstallation stopped. Close Axolotl Launcher and retry."
  ${EndIf}
  Push "error:$UninstallFailure"
  Call un.ReportInstallerProgress
  SetErrorLevel 2
FunctionEnd

Function un.onUninstSuccess
  Push 100
  Call un.ReportInstallerProgress
  SetErrorLevel 0
FunctionEnd

Section Uninstall

  Push 4
  Call un.ReportInstallerProgress
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  !ifmacrodef NSIS_HOOK_PREUNINSTALL
    !insertmacro NSIS_HOOK_PREUNINSTALL
  !endif

  Push 26
  Call un.ReportInstallerProgress
  !insertmacro AxlUninstallFile "$INSTDIR\${MAINBINARYNAME}.exe"

  ; Delete resources
  {{#each resources}}
    !insertmacro AxlUninstallFile "$INSTDIR\\{{this.[1]}}"
  {{/each}}

  ; Delete external binaries
  {{#each binaries}}
    !insertmacro AxlUninstallFile "$INSTDIR\\{{this}}"
  {{/each}}

  ; Keep the uninstaller and registration available if optional data removal fails.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RmDir /r "$APPDATA\${BUNDLEID}"
    Push "$APPDATA\${BUNDLEID}"
    Call un.CheckPathRemoved
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
    Push "$LOCALAPPDATA\${BUNDLEID}"
    Call un.CheckPathRemoved
  ${EndIf}

  Push 66
  Call un.ReportInstallerProgress
  ; Delete app associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
      !insertmacro APP_UNASSOCIATE "{{ext}}" "{{or association.name ext}}"
    {{/each}}
  {{/each}}

  ; Delete deep links
  {{#each deep_link_protocols as |protocol| ~}}
    ReadRegStr $R7 SHCTX "Software\Classes\\{{protocol}}\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey SHCTX "Software\Classes\\{{protocol}}"
    ${EndIf}
  {{/each}}


  {{#each resources_ancestors}}
  RMDir "$INSTDIR\\{{this}}"
  {{/each}}

  ; Remove shortcuts if not updating
  ${If} $UpdateMode <> 1
    !insertmacro DeleteAppUserModelId

    ; Remove start menu shortcut
    !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
    !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      !insertmacro AxlUninstallFile "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      RMDir "$SMPROGRAMS\$AppStartMenuFolder"
    ${EndIf}
    !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      !insertmacro AxlUninstallFile "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}

    ; Remove desktop shortcuts
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
      !insertmacro AxlUninstallFile "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}

  ; Removes the Autostart entry for ${PRODUCTNAME} from the HKCU Run key if it exists.
  ; This ensures the program does not launch automatically after uninstallation if it exists.
  ; If it doesn't exist, it does nothing.
  ; We do this when not updating (to preserve the registry value on updates)
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  ${EndIf}

  ; Delete app data if the checkbox is selected
  ; and if not updating
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ; Clear the install location $INSTDIR from registry
    DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty SHCTX "${MANUKEY}"

    ; Clear the install language from registry
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"
    DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty HKCU "${MANUKEY}"

  ${EndIf}

  !ifmacrodef NSIS_HOOK_POSTUNINSTALL
    !insertmacro NSIS_HOOK_POSTUNINSTALL
  !endif

  Push 82
  Call un.ReportInstallerProgress

  ; Legacy installers execute this file in place with _?=; allow deferred
  ; self-deletion there. The Webview child runs the already-created temp copy.
  ${If} $EXEPATH == "$INSTDIR\uninstall.exe"
    Delete /REBOOTOK "$INSTDIR\uninstall.exe"
  ${Else}
    !insertmacro AxlUninstallFile "$INSTDIR\uninstall.exe"
  ${EndIf}

  ClearErrors
  DeleteRegKey SHCTX "${UNINSTKEY}"
  ReadRegStr $0 SHCTX "${UNINSTKEY}" "UninstallString"
  ${IfNot} ${Errors}
    StrCpy $UninstallFailure "Could not remove the Windows uninstall registration"
    SetErrorLevel 2
    Abort
  ${EndIf}
  RMDir "$INSTDIR"

  ; Auto close if passive mode or updating
  ${If} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Function RestorePreviousInstallLocation
  ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
  StrCmp $4 "" +2 0
    StrCpy $INSTDIR $4
FunctionEnd

Function Skip
  Abort
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd
Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd

Function CreateOrUpdateStartMenuShortcut
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  StrCpy $R0 0

  !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    StrCpy $R0 1
  ${EndIf}

  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    StrCpy $R0 1
  ${EndIf}

  ${If} $R0 = 1
    Return
  ${EndIf}

  ; Skip creating shortcut if in update mode or no shortcut mode
  ; but always create if migrating from wix
  ${If} $WixMode = 0
    ${If} $UpdateMode = 1
    ${OrIf} $NoShortcutMode = 1
      Return
    ${EndIf}
  ${EndIf}

  !if "${STARTMENUFOLDER}" != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  !else
    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  !endif
FunctionEnd

Function CreateOrUpdateDesktopShortcut
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Return
  ${EndIf}

  ; Skip creating shortcut if in update mode or no shortcut mode
  ; but always create if migrating from wix
  ${If} $WixMode = 0
    ${If} $UpdateMode = 1
    ${OrIf} $NoShortcutMode = 1
    ${OrIf} $NoDesktopShortcutMode = 1
      Return
    ${EndIf}
  ${EndIf}

  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
FunctionEnd
