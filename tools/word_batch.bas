' Batch conversion inside Word: docx -> PDF, one file after another, without
' one AppleEvent per document. Word on this Mac stops answering AppleScript
' after a few documents (open returns before the document exists, then
' save as times out), while its own VBA keeps working (2026-09-20).
'
' Install once by hand: Word > Tools > Macro > Visual Basic Editor,
' Normal > Modules > insert a module, paste this file, save Normal.
' Run from a shell:
'   osascript -e 'tell application "Microsoft Word" to run VB macro macro name "BatchPdf"'
' It reads ~/Documents/officework-cmp/templates/batch.txt (one docx path per
' line), writes <path without .docx>.ms.pdf next to each file, skips files
' whose PDF exists, and appends a line per file to batch.log.
Sub BatchPdf()
    Dim home As String
    home = Environ("HOME")
    Dim listPath As String
    listPath = home & "/Documents/officework-cmp/templates/batch.txt"
    Dim logPath As String
    logPath = home & "/Documents/officework-cmp/templates/batch.log"
    Dim fIn As Integer, fLog As Integer
    fIn = FreeFile
    Open listPath For Input As #fIn
    fLog = FreeFile
    Open logPath For Append As #fLog
    Dim src As String, dst As String
    Dim d As Document
    Do While Not EOF(fIn)
        Line Input #fIn, src
        src = Trim(src)
        If Len(src) > 0 Then
            dst = Left(src, Len(src) - 5) & ".ms.pdf"
            If Dir(dst) = "" Then
                On Error Resume Next
                Set d = Documents.Open(FileName:=src, ReadOnly:=True, AddToRecentFiles:=False, Visible:=False)
                If Err.Number <> 0 Then
                    Print #fLog, "open failed" & vbTab & src & vbTab & Err.Description
                    Err.Clear
                Else
                    d.ExportAsFixedFormat OutputFileName:=dst, ExportFormat:=wdExportFormatPDF
                    If Err.Number <> 0 Then
                        Print #fLog, "export failed" & vbTab & src & vbTab & Err.Description
                        Err.Clear
                    Else
                        Print #fLog, "ok" & vbTab & src
                    End If
                    d.Close SaveChanges:=wdDoNotSaveChanges
                    Err.Clear
                End If
                On Error GoTo 0
            End If
        End If
    Loop
    Close #fIn
    Close #fLog
End Sub
