!ifndef HOSPITAL_NSIS_STARTUP
!define HOSPITAL_NSIS_STARTUP

!include LogicLib.nsh
!define HOSPITAL_STARTUP_REGISTRY_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

; The shared entry covers new accounts. Each session applies its own
; start_on_login preference before loading the transcription engine.
!macro HospitalRegisterStartup
    SetRegView 64
    ClearErrors
    WriteRegStr HKLM "${HOSPITAL_STARTUP_REGISTRY_KEY}" "${BUNDLEID}" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --autostart"
    SetRegView lastused
    ${If} ${Errors}
        DetailPrint "Impossible d'enregistrer le démarrage automatique de Murmure Vauclaire."
        MessageBox MB_ICONSTOP|MB_OK "Impossible d'enregistrer le démarrage automatique de Murmure Vauclaire." /SD IDOK
        SetErrorLevel 1603
        Abort
    ${EndIf}
!macroend

!macro HospitalRemoveStartup
    Push $0
    SetRegView 64
    ClearErrors
    ReadRegStr $0 HKLM "${HOSPITAL_STARTUP_REGISTRY_KEY}" "${BUNDLEID}"
    ${IfNot} ${Errors}
        ; Keep every other application's startup entry intact.
        DeleteRegValue HKLM "${HOSPITAL_STARTUP_REGISTRY_KEY}" "${BUNDLEID}"
        ${If} ${Errors}
            SetRegView lastused
            Pop $0
            DetailPrint "Impossible de supprimer le démarrage automatique de Murmure Vauclaire."
            MessageBox MB_ICONSTOP|MB_OK "Impossible de supprimer le démarrage automatique de Murmure Vauclaire." /SD IDOK
            SetErrorLevel 1603
            Abort
        ${EndIf}
    ${EndIf}
    SetRegView lastused
    Pop $0
    ClearErrors
!macroend

!endif
