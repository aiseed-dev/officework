# ODF→XLSX: `table:allow-empty-cell` of a content validation is never read

## Summary

A data validation rule that allows empty cells loses that setting. Every rule converted from an ods gets `allowBlank="0"`.

## Steps to reproduce

1. Take the attached `03-allow-empty-cell.ods`. A2 has a rule "whole number between 1 and 10" with `table:allow-empty-cell="true"`.
2. Convert it to xlsx with x2t.

## Actual result

```xml
<x14:dataValidation allowBlank="0" operator="between" ... type="whole">
```

## Expected result

`allowBlank="1"`. LibreOffice 24.2 writes `allowBlank="true"` for the same file.

## Cause

[`table_content_validation::add_attributes`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Format/table.cpp#L825) reads the attribute under a misspelt name:

```cpp
CP_APPLY_ATTR(L"table:allowempty-cell", table_allowempty_cell_);
```

The attribute is `table:allow-empty-cell`, as the ODS writer in [OdfFile/Writer/Format/table.cpp](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Writer/Format/table.cpp#L992) spells it. `table_content_validation::xlsx_convert()` also does not pass the value on, and `allowBlank` keeps its default `false`.

## Suggested fix

Read `L"table:allow-empty-cell"`, and set `allowBlank` from it in `xlsx_convert()` through `xlsx_dataValidations_context`.

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
