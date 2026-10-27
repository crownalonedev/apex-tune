; Apex Tune installer hooks (NSIS).
;
; Uninstalling removes Apex Tune's own sign-in tasks and stops the timer
; helper, so nothing is left pointing at deleted files. It does NOT revert
; tweaks: use "Revert everything" in Apex Tune before uninstalling.

!macro NSIS_HOOK_PREINSTALL
  ; An update replaces apex-timer.exe, so stop it first.
  nsExec::Exec 'taskkill /IM apex-timer.exe /F'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONQUESTION "Uninstalling doesn't undo Apex Tune's changes. To put every setting back first, choose No, open Apex Tune and use Revert everything.$\r$\n$\r$\nContinue uninstalling?" IDYES +2
  Abort
  nsExec::Exec 'taskkill /IM apex-timer.exe /F'
  nsExec::Exec 'schtasks /Delete /TN "\ApexTune\Timer" /F'
  nsExec::Exec 'schtasks /Delete /TN "\ApexTune\App" /F'
!macroend
