# ODF→XLSX: `<numFmts>` is written with a `formatCode` attribute instead of `count`

## Summary

Every xlsx converted from an ods gets `<numFmts formatCode="N">` in styles.xml. Strict readers refuse the file.

## Steps to reproduce

1. Take the attached `02-numfmts-count.ods` (one cell with a percent format). Any ods shows the same.
2. Convert it to xlsx with x2t.
3. Look at `xl/styles.xml`, or open the file with openpyxl.

## Actual result

```xml
<numFmts formatCode="0"/>
```

openpyxl stops with `NumberFormatList.__init__() got an unexpected keyword argument 'formatCode'`.

## Expected result

```xml
<numFmts count="N">...</numFmts>
```

ECMA-376 Part 1, 18.8.31 numFmts has the attribute `count` (the number of `numFmt` children). `formatCode` is an attribute of `numFmt`, not of `numFmts`. LibreOffice writes `count` (sc/source/filter/excel/xestyle.cxx).

## Cause

[`xlsx_num_fmts::Impl::serialize`](https://github.com/Euro-Office/core/blob/25ea5148473429657a64afba20d21e8454e93fde/OdfFile/Reader/Converter/xlsx_numFmts.cpp#L168):

```cpp
CP_XML_NODE (L"numFmts")
{
    CP_XML_ATTR (L"formatCode", arrFormats.size());
```

## Suggested fix

```cpp
CP_XML_ATTR (L"count", arrFormats.size());
```

## Versions

Reproduced with the x2t of ONLYOFFICE Desktop Editors 9.4.0 (flatpak). The same code is in Euro-Office core main at 25ea5148.
