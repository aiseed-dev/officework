# ODF→XLSX: a conditional format whose value contains "top", "bottom" or "duplicate" gets the wrong rule type

## Summary

A conditional format "cell value is equal to "Desktop"" in an ods becomes a top 10 rule in the xlsx.

## Steps to reproduce

1. Take the attached `01-cf-value-containing-top.ods`. Cells A1:A3 hold Desktop, Laptop and Mobile. The range has one rule, written the way LibreOffice writes it:
   ```xml
   <calcext:condition calcext:apply-style-name="Hit" calcext:value="=&quot;Desktop&quot;" calcext:base-cell-address="Sheet1.A1"/>
   ```
2. Convert it to xlsx with x2t, or open it in the desktop editors and save it as xlsx.

## Actual result

```xml
<conditionalFormatting sqref="A1:A3"><cfRule priority="1" type="top10"/></conditionalFormatting>
```

## Expected result

A `cellIs` rule, as LibreOffice 24.2 writes when it saves the same file as xlsx:

```xml
<cfRule type="cellIs" operator="equal" ...><formula>"Desktop"</formula></cfRule>
```

## Cause

[`xlsx_conditionalFormatting_context::set_formula`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Converter/xlsx_conditionalFormatting.cpp#L686) chooses the rule type by looking for a word anywhere in the value with `f.find(...)`. It tries "duplicate" (line 617), "begins-with", "contains-text", "top" (line 686) and "bottom" (line 700) before it falls back to `cellIs` (line 717). `f.find(L"top")` matches the "top" in "Desktop". Values with "Laptop", "stop", "bottom line", "duplicate", "begins-with" and so on are taken the same way.

## Suggested fix

Match the function names only at the start of the value (`top-elements(`, `bottom-elements(`, `top-percent(`, `bottom-percent(`, `begins-with(` and so on). A value that starts with a comparison operator (`<`, `>`, `<=`, `>=`, `=`, `!=`) is a `cellIs` rule.

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
