; Runs inside the Tauri NSIS installer.

!macro NSIS_HOOK_PREUNINSTALL
  ; Start with Windows registers an elevated task; don't leave it behind.
  ; An update (/UPDATE) keeps it: the new version starts the same way.
  ${If} $UpdateMode <> 1
    nsExec::Exec 'schtasks /Delete /TN MultiPath /F'
  ${EndIf}
!macroend
