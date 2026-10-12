# Close the Word and Excel that a run of ms_pdf.ps1 started, without killing them.
#
# Runs as the signed-in user. Only processes started at or after -Since are
# touched, so a Word or Excel the user opened before the run is left alone.
# A window gets the close button's message; a hidden instance (no window) is
# reached over COM and quit without saving. Writes <Root>\close.txt.
param([string]$Root = 'C:\owcmp', [string]$Since = '')

$log = Join-Path $Root 'close.txt'
Set-Content -Encoding UTF8 $log ('close ' + (Get-Date -Format s))
function Log($s) { Add-Content -Encoding UTF8 $log $s }
$from = if ($Since) { [datetime]::Parse($Since) } else { [datetime]::MinValue }

function Ours($name) {
  @(Get-Process $name -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -ge $from })
}

foreach ($p in (Ours 'WINWORD') + (Ours 'EXCEL')) {
  if ($p.MainWindowHandle -ne 0) {
    Log ('window ' + $p.ProcessName + ' ' + $p.Id + ' close=' + $p.CloseMainWindow())
  }
}
Start-Sleep 5

# Hidden instances: quit over COM. The documents were opened read-only and not
# changed, so nothing is saved
if ((Ours 'WINWORD').Count -gt 0) {
  try {
    $w = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
    foreach ($d in @($w.Documents)) { $d.Close(0) }
    $w.Quit()
    Log 'word quit'
  } catch { Log ('word ' + $_.Exception.Message) }
}
if ((Ours 'EXCEL').Count -gt 0) {
  try {
    $x = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
    $x.DisplayAlerts = $false
    foreach ($b in @($x.Workbooks)) { $b.Close($false) }
    $x.Quit()
    Log 'excel quit'
  } catch { Log ('excel ' + $_.Exception.Message) }
}
Start-Sleep 5
foreach ($p in (Ours 'WINWORD') + (Ours 'EXCEL')) { Log ('left ' + $p.ProcessName + ' ' + $p.Id) }
