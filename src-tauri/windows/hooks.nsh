; Runs inside the Tauri NSIS installer.

!macro NSIS_HOOK_PREUNINSTALL
  ; Start with Windows registers an elevated task; don't leave it behind.
  nsExec::Exec 'schtasks /Delete /TN MultiPath /F'
!macroend
