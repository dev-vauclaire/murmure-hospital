!ifndef HOSPITAL_NSIS_INSTALLATION_MARKER
!define HOSPITAL_NSIS_INSTALLATION_MARKER

!include LogicLib.nsh

; Tauri defines MANUPRODUCTKEY from publisher and productName after including
; hooks.nsh. Resolve it when these macros are expanded, inside the install steps.
!macro HospitalWriteInstallationMarker
    SetRegView 64
    ClearErrors
    WriteRegDWORD HKLM "${MANUPRODUCTKEY}" "isInstalled" 1
    SetRegView lastused
    ${If} ${Errors}
        DetailPrint "Impossible d'enregistrer l'indicateur d'installation de Murmure Vauclaire."
        MessageBox MB_ICONSTOP|MB_OK "Impossible d'enregistrer l'indicateur d'installation de Murmure Vauclaire." /SD IDOK
        SetErrorLevel 1603
        Abort
    ${EndIf}
!macroend

!macro HospitalRemoveInstallationMarker
    Push $0
    ; Tauri may have switched to the current user's context while deleting data.
    SetRegView 64
    ClearErrors
    ReadRegDWORD $0 HKLM "${MANUPRODUCTKEY}" "isInstalled"
    ; The native uninstaller may already have removed the entire application key.
    ${IfNot} ${Errors}
        DeleteRegValue HKLM "${MANUPRODUCTKEY}" "isInstalled"
        ${If} ${Errors}
            SetRegView lastused
            Pop $0
            DetailPrint "Impossible de supprimer l'indicateur d'installation de Murmure Vauclaire."
            MessageBox MB_ICONSTOP|MB_OK "Impossible de supprimer l'indicateur d'installation de Murmure Vauclaire." /SD IDOK
            SetErrorLevel 1603
            Abort
        ${EndIf}
    ${EndIf}
    SetRegView lastused
    Pop $0
    ClearErrors
!macroend

!endif
