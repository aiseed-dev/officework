# ODF→XLSX: empty rows at the end of a sheet are merged even when they carry validations or different heights

## Summary

`table_rows::xlsx_convert` folds empty rows at the end of a table into the row before them. This has two problems.

1. Rows whose cells only carry a data validation are folded. The first row's rule then covers all of them, and the other rules are lost.
2. The row style check compares the last row with itself, so rows of different heights are folded too, and their heights are lost.

## Steps to reproduce

1. Take the attached `06a-rules-on-empty-cells.ods`. The empty cells A1, A2 and A3 each have a different rule: whole number between 1 and 11, between 2 and 12, and between 3 and 13.
2. Take the attached `06b-row-heights.ods`. Row 1 is 0.5 cm high and holds a value. Rows 2 and 3 are empty and are 2 cm and 4 cm high.
3. Convert both to xlsx with x2t.

## Actual result

06a: one rule on A1:A3, and the other two rules are gone.

```xml
<x14:dataValidation ... type="whole"><x14:formula1><xm:f>1</xm:f></x14:formula1><x14:formula2><xm:f>11</xm:f></x14:formula2><xm:sqref>A1:A3</xm:sqref></x14:dataValidation>
```

06b: row 3 has lost its height.

```xml
<row r="1" ht="14.172" customHeight="1">
<row r="2" ht="56.689" customHeight="1">
```

## Expected result

06a: three rules, on A1, A2 and A3. 06b: row 3 at 113.4 pt. LibreOffice 24.2 writes both that way when it saves the same files as xlsx.

## Cause

In [`table_rows::xlsx_convert`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Format/table_xlsx.cpp#L276), the loop from line 282 to line 296 has two faults.

1. Both style names are taken from the last row (lines 287 and 288):
   ```cpp
   std::wstring style   = row_last->attlist_.table_style_name_.get_value_or(L"");
   std::wstring style_1 = row_last->attlist_.table_style_name_.get_value_or(L"");
   ```
   The second one should read `row_last_1`.
2. [`table_table_cell::empty`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Format/table.cpp#L442) does not look at `table:content-validation-name`. A cell that only carries a rule counts as empty.

## Suggested fix

- Use `row_last_1` for `style_1`.
- Treat a cell with `table:content-validation-name` as not empty in this check.

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
