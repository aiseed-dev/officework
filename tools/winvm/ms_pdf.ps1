# Save every docx / xlsx in <Root>\in as PDF with Word / Excel (<Root>\out\<name>.ms.pdf).
#
# tools/winvm_pdf.py copies this into the Windows VM and runs it as the
# signed-in user (a scheduled task), because Word and Excel driven from the
# guest agent's SYSTEM account have no user profile or licence. It writes
# <Root>\log.txt as it goes and <Root>\done.txt at the end.
#
# What we ran into (2026-10-12, Word and Excel 16.0.20430 on Windows 11):
# * Pass every path to Word / Excel as [string]. A path from Join-Path is a
#   string wrapped in a PSObject, and Word's Documents.Open and
#   ExportAsFixedFormat given one never returned (Word kept working on screen).
# * Open read-only and never change the document, so closing it asks nothing.
# * A Word that was killed leaves recovered files behind. The next Word asks
#   what to do with them when it quits, and a hidden Word waits for that answer
#   for ever. So never kill Word; tools/winvm/close.ps1 closes it instead.
param([string]$Root = 'C:\owcmp')

$in = Join-Path $Root 'in'
$out = Join-Path $Root 'out'
$log = Join-Path $Root 'log.txt'
New-Item -ItemType Directory -Force $out | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $Root 'done.txt')
Set-Content -Encoding UTF8 $log ('start ' + (Get-Date -Format s))
function Log($s) { Add-Content -Encoding UTF8 $log ((Get-Date -Format HH:mm:ss) + ' ' + $s) }

$docs = @(Get-ChildItem $in -Filter *.docx)
$books = @(Get-ChildItem $in -Filter *.xlsx)

if ($docs.Count -gt 0) {
  $word = New-Object -ComObject Word.Application
  try {
    $word.DisplayAlerts = 0
    $word.Visible = $false
    foreach ($f in $docs) {
      [string]$src = $f.FullName
      [string]$pdf = Join-Path $out ($f.BaseName + '.ms.pdf')
      Remove-Item -Force -ErrorAction SilentlyContinue $pdf
      try {
        Log ('open ' + $f.Name)
        # Open(FileName, ConfirmConversions, ReadOnly, AddToRecentFiles)
        $d = $word.Documents.Open($src, $false, $true, $false)
        $pages = $d.ComputeStatistics(2)
        # 17 = wdExportFormatPDF
        $d.ExportAsFixedFormat($pdf, 17)
        $d.Close(0)
        Log ('ok   ' + $f.Name + ' pages=' + $pages)
      } catch {
        Log ('fail ' + $f.Name + ' ' + $_.Exception.Message)
      }
    }
  } finally {
    $word.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($word)
  }
}

if ($books.Count -gt 0) {
  $excel = New-Object -ComObject Excel.Application
  try {
    $excel.DisplayAlerts = $false
    $excel.Visible = $false
    foreach ($f in $books) {
      [string]$src = $f.FullName
      [string]$pdf = Join-Path $out ($f.BaseName + '.ms.pdf')
      Remove-Item -Force -ErrorAction SilentlyContinue $pdf
      try {
        Log ('open ' + $f.Name)
        # Open(FileName, UpdateLinks, ReadOnly)
        $b = $excel.Workbooks.Open($src, 0, $true)
        # 0 = xlTypePDF
        $b.ExportAsFixedFormat(0, $pdf)
        $b.Close($false)
        Log ('ok   ' + $f.Name)
      } catch {
        Log ('fail ' + $f.Name + ' ' + $_.Exception.Message)
      }
    }
  } finally {
    $excel.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
  }
}
Log 'end'
Set-Content (Join-Path $Root 'done.txt') 'done'
