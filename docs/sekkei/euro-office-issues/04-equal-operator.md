# ODF→XLSX: a content validation "equal to" written as `cell-content()=5` loses its operator and value

## Summary

LibreOffice writes "equal to" in a validation condition as `=`. x2t only looks for `==`, so the rule loses its operator and its value.

## Steps to reproduce

1. Take the attached `04-equal-operator.ods`. It has two rules written the way LibreOffice writes them:
   - A1: `of:cell-content-is-whole-number() and cell-content()=5`
   - A2: `of:cell-content-text-length()=3`
2. Convert it to xlsx with x2t.

## Actual result

```xml
<x14:dataValidation allowBlank="0" ... type="whole" errorStyle="stop"><xm:sqref>A1</xm:sqref></x14:dataValidation>
<x14:dataValidation allowBlank="0" ... type="textLength" errorStyle="stop"><xm:sqref>A2</xm:sqref></x14:dataValidation>
```

There is no `operator` and no `formula1`.

## Expected result

`operator="equal"` with `formula1` 5 and 3, as LibreOffice 24.2 writes when it saves the same file as xlsx.

## Cause

[`xlsx_dataValidations_context::add_formula`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Converter/xlsx_data_validation.cpp#L493) tests `()==`, `()!=`, `()<=`, `()<`, `()>=` and `()>`, but not `()=`.

The operators of the condition grammar are `<`, `>`, `<=`, `>=`, `=` and `!=` (ODF 1.2 Part 1, 19.596 table:condition). `==` is not one of them. The ODS writer writes `()==` for equal ([ods_table_context.cpp line 354](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Writer/Format/ods_table_context.cpp#L354)). LibreOffice happens to read that, because it takes `=` as the operator and `=5` as the expression.

## Suggested fix

- In `add_formula()`, accept `()=` after the two-character operators have been tested.
- In `ods_table_context.cpp`, write `()=` for equal.

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
