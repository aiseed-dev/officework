"""officework-mcp の `--panel` の印から出す run_macro を選ぶ(2026-09-05)。

表のパネル(`--panel` / `--panel=sheet`)は表の run_macro、文書のパネル
(`--panel=doc`)は文書の run_macro、印が無ければ出さない。
"""

from officework import mcp as m


def test_panel_flag_picks_the_macro_tool():
    assert m.panel_macro_tool(["--panel"]) is m.run_macro
    assert m.panel_macro_tool(["--panel=sheet"]) is m.run_macro
    assert m.panel_macro_tool(["--panel=doc"]) is m.doc_run_macro
    assert m.panel_macro_tool([]) is None


def test_the_document_macro_explains_src_and_out():
    doc = m.doc_run_macro.__doc__
    assert "src" in doc and "out" in doc and "python-docx" in doc
    assert "`b`" in m.run_macro.__doc__


if __name__ == "__main__":
    test_panel_flag_picks_the_macro_tool()
    test_the_document_macro_explains_src_and_out()
    print("ok")
