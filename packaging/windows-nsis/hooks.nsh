!ifndef HOSPITAL_NSIS_HOOKS
!define HOSPITAL_NSIS_HOOKS

; Keep the entry point directory stable while nested includes are expanded.
!define HOSPITAL_NSIS_HOOK_DIRECTORY "${__FILEDIR__}"
!include "${HOSPITAL_NSIS_HOOK_DIRECTORY}\hooks\prerequisites.nsh"
!include "${HOSPITAL_NSIS_HOOK_DIRECTORY}\hooks\installation-marker.nsh"

!macro NSIS_HOOK_PREINSTALL
    !insertmacro HospitalVcPreInstall
!macroend

!macro NSIS_HOOK_POSTINSTALL
    ; The prerequisite completion hook may quit with 3010 in silent/passive mode.
    !insertmacro HospitalWriteInstallationMarker
    !insertmacro HospitalVcPostInstall
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
    !insertmacro HospitalRemoveInstallationMarker
!macroend

!endif
