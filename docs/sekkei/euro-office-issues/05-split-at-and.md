# ODF→XLSX: content validation conditions are split at every "and", breaking lists and formulas

## Summary

A list validation with the items Grand, Hand and Stand becomes the list `"G`.

## Steps to reproduce

1. Take the attached `05-split-at-and.ods`. A1 has the rule:
   ```
   of:cell-content-is-in-list("Grand";"Hand";"Stand")
   ```
2. Convert it to xlsx with x2t.

## Actual result

```xml
<x14:dataValidation ... type="list"><x14:formula1><xm:f>&quot;&quot;G&quot;</xm:f></x14:formula1><xm:sqref>A1</xm:sqref></x14:dataValidation>
```

## Expected result

`<formula1>"Grand,Hand,Stand"</formula1>`, as LibreOffice 24.2 writes when it saves the same file as xlsx.

## Cause

[`xlsx_dataValidations_context::add_formula`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Converter/xlsx_data_validation.cpp#L409):

```cpp
boost::algorithm::split_regex(arrFormula, formula, boost::wregex(L"and"));
```

This splits the condition at the letters "and" anywhere, also inside quoted list items and inside string literals of a formula. A formula rule such as `of:is-true-formula([.A1]<>"Brand")` is cut the same way.

## Suggested fix

Split only once, at `" and "` outside quotes and brackets. Another way is to match the leading type function (`cell-content-is-whole-number()` and the others) and take the rest after `" and "`.

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
