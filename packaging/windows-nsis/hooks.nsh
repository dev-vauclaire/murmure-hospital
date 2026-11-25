!ifndef HOSPITAL_NSIS_VC_HOOKS
!define HOSPITAL_NSIS_VC_HOOKS

!include LogicLib.nsh
!include WordFunc.nsh

; Capture this directory while the hook is included, before Tauri expands macros.
!define HOSPITAL_VC_HOOK_DIRECTORY "${__FILEDIR__}"
!include "${HOSPITAL_VC_HOOK_DIRECTORY}/../../src-tauri/target/nsis-prerequisites/vc-redist.nsh"
!define /ifndef MUI_FINISHPAGE_REBOOTLATER_DEFAULT
!define HOSPITAL_VC_REGISTRY_KEY "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64"

; Input: version string on the stack. Output: 1 if sufficiently recent, else 0.
; Validate first: VersionCompare alone accepts malformed non-numeric components.
Function HospitalVcVersionIsSufficient
    Exch $0
    Push $1
    Push $2
    Push $3
    Push $4
    Push $5

    StrCpy $1 $0 1
    ${If} $1 == "v"
    ${OrIf} $1 == "V"
        StrCpy $0 $0 "" 1
    ${EndIf}
    StrCpy $1 0
    StrCpy $2 0
    StrCpy $3 0
    StrCpy $5 0
    hospital_vc_version_next:
        StrCpy $4 $0 1 $1
        StrCmp $4 "" hospital_vc_version_complete
        StrCmp $4 "." hospital_vc_version_separator
        StrCmp $4 "0" hospital_vc_version_digit
        StrCmp $4 "1" hospital_vc_version_digit
        StrCmp $4 "2" hospital_vc_version_digit
        StrCmp $4 "3" hospital_vc_version_digit
        StrCmp $4 "4" hospital_vc_version_digit
        StrCmp $4 "5" hospital_vc_version_digit
        StrCmp $4 "6" hospital_vc_version_digit
        StrCmp $4 "7" hospital_vc_version_digit
        StrCmp $4 "8" hospital_vc_version_digit
        StrCmp $4 "9" hospital_vc_version_digit
        Goto hospital_vc_version_invalid
    hospital_vc_version_digit:
        IntOp $2 $2 + 1
        IntCmp $2 5 0 0 hospital_vc_version_invalid
        IntOp $5 $5 * 10
        IntOp $5 $5 + $4
        IntCmp $5 65535 0 0 hospital_vc_version_invalid
        Goto hospital_vc_version_advance
    hospital_vc_version_separator:
        StrCmp $2 0 hospital_vc_version_invalid
        IntOp $3 $3 + 1
        IntCmp $3 3 0 0 hospital_vc_version_invalid
        StrCpy $2 0
        StrCpy $5 0
    hospital_vc_version_advance:
        IntOp $1 $1 + 1
        Goto hospital_vc_version_next
    hospital_vc_version_complete:
        StrCmp $2 0 hospital_vc_version_invalid
        ${VersionCompare} "$0" "${HOSPITAL_VC_REDIST_VERSION}" $1
        StrCmp $1 2 hospital_vc_version_invalid
        StrCpy $0 1
        Goto hospital_vc_version_done
    hospital_vc_version_invalid:
        StrCpy $0 0
    hospital_vc_version_done:
        Pop $5
        Pop $4
        Pop $3
        Pop $2
        Pop $1
        Exch $0
FunctionEnd

!macro HospitalVcReadRegistryView VIEW
    SetRegView ${VIEW}
    ReadRegDWORD $0 HKLM "${HOSPITAL_VC_REGISTRY_KEY}" "Installed"
    ReadRegStr $1 HKLM "${HOSPITAL_VC_REGISTRY_KEY}" "Version"
    SetRegView lastused
    ${If} $0 == 1
        Push $1
        Call HospitalVcVersionIsSufficient
        Pop $2
        ${If} $2 == 1
            StrCpy $3 1
        ${EndIf}
    ${EndIf}
!macroend

; Read both views independently and restore Tauri's current registry view.
Function HospitalVcRuntimeIsAvailable
    Push $0
    Push $1
    Push $2
    Push $3
    StrCpy $3 0
    !insertmacro HospitalVcReadRegistryView 64
    !insertmacro HospitalVcReadRegistryView 32
    StrCpy $0 $3
    Pop $3
    Pop $2
    Pop $1
    Exch $0
    ClearErrors
FunctionEnd

; Input: redistributable exit code. Abort installation on an actual failure.
Function HospitalVcHandleExitCode
    Exch $0
    ${If} $0 == 3010
        SetRebootFlag true
        DetailPrint "Visual C++ x64 installé. Un redémarrage sera nécessaire."
    ${ElseIf} $0 != 0
        DetailPrint "Échec de l'installation de Visual C++ x64 : code $0."
        MessageBox MB_ICONSTOP|MB_OK "L'installation de Visual C++ x64 a échoué (code $0). Murmure Vauclaire ne sera pas installé." /SD IDOK
        SetErrorLevel $0
        Pop $0
        Abort
    ${EndIf}
    Pop $0
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
    Push $0
    DetailPrint "Vérification de Visual C++ x64..."
    Call HospitalVcRuntimeIsAvailable
    Pop $0
    ${If} $0 == 1
        DetailPrint "Une version suffisante de Visual C++ x64 est déjà installée."
    ${Else}
        InitPluginsDir
        File "/oname=$PLUGINSDIR\VC_redist.x64.exe" "${HOSPITAL_VC_REDIST_FILE}"
        DetailPrint "Installation du prérequis Visual C++ x64 embarqué..."
        ClearErrors
        ExecWait '"$PLUGINSDIR\VC_redist.x64.exe" /install /quiet /norestart' $0
        ${If} ${Errors}
            StrCpy $0 1603
        ${EndIf}
        Delete "$PLUGINSDIR\VC_redist.x64.exe"
        Push $0
        Call HospitalVcHandleExitCode
    ${EndIf}
    Pop $0
    ClearErrors
!macroend

!macro NSIS_HOOK_POSTINSTALL
    ; Tauri handles /R in .onInstSuccess even when a reboot is pending. Quit
    ; here only in silent/passive mode, after the application was installed.
    ${If} ${RebootFlag}
        ${If} $PassiveMode == 1
        ${OrIf} ${Silent}
            SetErrorLevel 3010
            Quit
        ${EndIf}
    ${EndIf}
!macroend

!endif
