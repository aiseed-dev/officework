"""Write the small ods files that reproduce the Euro-Office issues.

    .venv/bin/python docs/sekkei/euro-office-issues/make_files.py docs/sekkei/euro-office-issues

Each file holds only what its issue needs, written the way LibreOffice 24.2
writes it. LibreOffice reads each one as intended (checked by saving them
as xlsx with tools/lo_pdf.py --to xlsx); x2t shows the fault.
"""
import sys, zipfile, pathlib
out = pathlib.Path(sys.argv[1])
NS = ('xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
      'xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" '
      'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
      'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
      'xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" '
      'xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" '
      'xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" '
      'office:version="1.3"')
MAN = ('<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">'
       '<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>'
       '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
       '<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/></manifest:manifest>')
def save(name, body, auto="", named="", validations=""):
    content = (f'<?xml version="1.0" encoding="UTF-8"?><office:document-content {NS}>'
               f'<office:automatic-styles>{auto}</office:automatic-styles><office:body><office:spreadsheet>'
               f'{validations}<table:table table:name="Sheet1">{body}</table:table></office:spreadsheet></office:body></office:document-content>')
    styles = (f'<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NS}><office:styles>'
              f'<style:style style:name="Default" style:family="table-cell"/>{named}</office:styles></office:document-styles>')
    with zipfile.ZipFile(out / name, "w") as z:
        z.writestr("mimetype", "application/vnd.oasis.opendocument.spreadsheet", compress_type=zipfile.ZIP_STORED)
        z.writestr("content.xml", content, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("styles.xml", styles, compress_type=zipfile.ZIP_DEFLATED)
        z.writestr("META-INF/manifest.xml", MAN, compress_type=zipfile.ZIP_DEFLATED)
def s(t):
    return f'<table:table-cell office:value-type="string"><text:p>{t}</text:p></table:table-cell>'
def row(*cells):
    return '<table:table-row>' + ''.join(cells) + '</table:table-row>'
def rule(name, cond, empty="true"):
    return (f'<table:content-validation table:name="{name}" table:condition="{cond}" '
            f'table:allow-empty-cell="{empty}" table:base-cell-address="Sheet1.A1"/>')

# 1: a conditional format "cell value is equal to "Desktop""
save("01-cf-value-containing-top.ods",
     '<table:table-column table:number-columns-repeated="1"/>' + row(s("Desktop")) + row(s("Laptop")) + row(s("Mobile")) +
     '<calcext:conditional-formats><calcext:conditional-format calcext:target-range-address="Sheet1.A1:Sheet1.A3">'
     '<calcext:condition calcext:apply-style-name="Hit" calcext:value="=&quot;Desktop&quot;" calcext:base-cell-address="Sheet1.A1"/>'
     '</calcext:conditional-format></calcext:conditional-formats>',
     named='<style:style style:name="Hit" style:family="table-cell" style:parent-style-name="Default">'
           '<style:table-cell-properties fo:background-color="#ffcc00"/></style:style>')

# 2: a cell with a percent format
save("02-numfmts-count.ods",
     '<table:table-column/>' + row('<table:table-cell table:style-name="ce1" office:value-type="percentage" office:value="0.5"><text:p>50%</text:p></table:table-cell>'),
     auto='<number:percentage-style style:name="N1"><number:number number:decimal-places="0" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>'
          '<style:style style:name="ce1" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N1"/>')

# 3: a rule that allows empty cells
save("03-allow-empty-cell.ods",
     '<table:table-column/>' + row(s("x"), ) + row('<table:table-cell table:content-validation-name="val1" office:value-type="float" office:value="5"><text:p>5</text:p></table:table-cell>'),
     validations='<table:content-validations>' + rule("val1", "of:cell-content-is-whole-number() and cell-content-is-between(1,10)") + '</table:content-validations>')

# 4: "equal to" rules as LibreOffice writes them
save("04-equal-operator.ods",
     '<table:table-column/>' + row('<table:table-cell table:content-validation-name="val1" office:value-type="float" office:value="5"><text:p>5</text:p></table:table-cell>')
     + row('<table:table-cell table:content-validation-name="val2" office:value-type="string"><text:p>abc</text:p></table:table-cell>'),
     validations='<table:content-validations>' + rule("val1", "of:cell-content-is-whole-number() and cell-content()=5")
     + rule("val2", "of:cell-content-text-length()=3") + '</table:content-validations>')

# 5: list items that contain "and"
save("05-split-at-and.ods",
     '<table:table-column/>' + row('<table:table-cell table:content-validation-name="val1" office:value-type="string"><text:p>Hand</text:p></table:table-cell>'),
     validations='<table:content-validations>' + rule("val1", "of:cell-content-is-in-list(&quot;Grand&quot;;&quot;Hand&quot;;&quot;Stand&quot;)") + '</table:content-validations>')

# 6a: three different rules on empty cells A1:A3
save("06a-rules-on-empty-cells.ods",
     '<table:table-column table:number-columns-repeated="2"/>' + ''.join(
         row(f'<table:table-cell table:content-validation-name="val{i}"/>', '<table:table-cell/>') for i in (1, 2, 3)),
     validations='<table:content-validations>' + ''.join(
         rule(f"val{i}", f"of:cell-content-is-whole-number() and cell-content-is-between({i},{i+10})") for i in (1, 2, 3)) + '</table:content-validations>')

# 6b: empty rows of different heights at the end of the sheet
save("06b-row-heights.ods",
     '<table:table-column/>'
     '<table:table-row table:style-name="ro1"><table:table-cell office:value-type="float" office:value="1"><text:p>1</text:p></table:table-cell></table:table-row>'
     '<table:table-row table:style-name="ro2"><table:table-cell/></table:table-row>'
     '<table:table-row table:style-name="ro3"><table:table-cell/></table:table-row>',
     auto=''.join(f'<style:style style:name="ro{i}" style:family="table-row"><style:table-row-properties style:row-height="{h}cm" style:use-optimal-row-height="false"/></style:style>'
                  for i, h in ((1, 0.5), (2, 2.0), (3, 4.0))))
