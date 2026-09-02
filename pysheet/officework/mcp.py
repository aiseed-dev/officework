"""officework を **MCP の道具**として差し出す(2026-08-15)。

**向きが逆になる。** これまでは officework が AI を呼んでいた。ここでは
**AI の側**(利用者が動かしている Claude Code / Claude Desktop / その他の
MCP の客)が officework を道具として使う。

    「いま開いている表の B 列を合計して、下に置いて」
    「申込書の氏名と住所を埋めて、PDF にして」

と利用者が自分の AI に言えば、その AI がここの道具を呼んで officework を
操ります。**表の道具と文書の道具があります**(文書の方は `doc_` で始まる)。

# なぜこの形か

1. **契約の話が消える。** 枠を使うのは利用者と、その人が選んだ AI の
   製品との間。officework は「ログインと枠を提供する」場面を持たない
2. **どの客からでも使える。** Claude Code でも Claude Desktop でも、
   MCP を話す物なら何でも
3. **薄い。** 芯は既にある — 表は AF_UNIX の受け口(ops の 52 命令)を
   持ち、`officework.calc` がその橋。文章にも同じ受け口があります。
   ここはそれを道具の名前で包むだけ

# 使い方

    pip install "officework[mcp]"

客の設定に足す(Claude Code なら `claude mcp add`、Claude Desktop なら
設定ファイル):

    officework-mcp

**officework が動いている必要がある。** 動いていなければ、そう言って
断ります(黙って空を返しません)。文書の道具を呼んだのに表が前に出て
いるときも、そう言います。

# 何を渡していないか

- **任意のコードを走らせる道具は置かない。** 「Python を実行」を1つ
  置けば何でもできてしまい、利用者が許した範囲を越える
- 触るのは**いま開いている物**だけ。ファイルを開く道具は置いていません
  — 何を開くかは人が officework の側で決めます
- 保存は人が頼んだときだけ(`save`)。勝手に上書きしません
"""

from __future__ import annotations

import os

# **mcp は 2.0 で置き場と名前が変わりました。** `FastMCP` は
# `mcp.server.mcpserver.MCPServer` になっています。道具の付け方
# (`@…tool()`)も走らせ方(`.run()`)も同じなので、両方から探します。
# 見つからないときは、*入っていないのか古い/新しいのか*を分けて言います
try:
    from mcp.server.mcpserver import MCPServer as _Server      # mcp 2.x
except ModuleNotFoundError:
    try:
        from mcp.server.fastmcp import FastMCP as _Server      # mcp 1.x
    except ModuleNotFoundError as e:  # pragma: no cover - 入れていない人向け
        if e.name == "mcp":
            raise SystemExit(
                "MCP の口には mcp が要ります: pip install \"officework[mcp]\""
            ) from e
        raise SystemExit(
            "入っている mcp が合いません。1.x か 2.x を入れてください: "
            "pip install -U \"officework[mcp]\""
        ) from e

from . import calc as xw

mcp = _Server("officework")


def _book():
    """いま calc に出ているブック。**動いていなければそう言う**"""
    try:
        return xw.Book.attach()
    except Exception as e:
        raise RuntimeError(
            f"calc に繋がりません({e})。calc を起動してから使ってください"
        ) from e


def _sheet(name: str | None):
    wb = _book()
    return wb.sheets[name] if name else wb.sheets.active


@mcp.tool()
def book_info() -> dict:
    """いま開いているブックの様子(名前・径路・シートの一覧・選択範囲)。

    **最初にこれを呼ぶ。** どのシートに何があるかを知らずに書き込まない。
    """
    wb = _book()
    return {
        "name": wb.name,
        "path": wb.fullname,
        "sheets": wb.sheet_names,
        "active": _sheet(None).name,
        "selection": wb.selection.address,
    }


@mcp.tool()
def used_range(sheet: str | None = None) -> str:
    """そのシートで**使われている範囲**の番地(例 `A1:F42`)。空なら `A1`"""
    return _sheet(sheet).used_range.address


@mcp.tool()
def read_range(a1: str, sheet: str | None = None) -> list:
    """範囲の値を読む(2次元の並びで返す)。

    `a1` は `A1` でも `A1:C9` でもよい。**大きすぎる範囲は避ける** —
    まず `used_range` で広さを見てから。
    """
    v = _sheet(sheet)[a1].value
    return v if isinstance(v, list) else [[v]]


@mcp.tool()
def read_formulas(a1: str, sheet: str | None = None) -> list:
    """範囲の**式**を読む(値ではなく `=SUM(...)` の方)。空欄は空の字"""
    v = _sheet(sheet)[a1].formula
    return v if isinstance(v, list) else [[v]]


@mcp.tool()
def write_range(a1: str, values: list, sheet: str | None = None) -> str:
    """範囲に値を書く。`values` は2次元の並び(1行でも `[[...]]`)。

    **`=` で始まる字は式として入る。** 消したいセルには空の字を置く。
    書いた跡は calc の Ctrl+Z で戻せる。
    """
    if values and not isinstance(values[0], list):
        values = [values]
    _sheet(sheet)[a1].value = values
    return f"{a1} に {len(values)} 行を書きました"


@mcp.tool()
def set_format(
    a1: str,
    sheet: str | None = None,
    bold: bool | None = None,
    italic: bool | None = None,
    number_format: str | None = None,
    fill: str | None = None,
) -> str:
    """範囲に書式を掛ける。指定した物だけが変わる。

    - `number_format`: `#,##0` `¥#,##0` `0.00%` `yyyy/m/d` など
    - `fill`: `FFF2CC` のような RRGGBB。空の字で塗りを消す
    """
    r = _sheet(sheet)[a1]
    直した = []
    if bold is not None:
        r.font.bold = bold
        直した.append("太字")
    if italic is not None:
        r.font.italic = italic
        直した.append("斜体")
    if number_format is not None:
        r.number_format = number_format
        直した.append("表示形式")
    if fill is not None:
        r.color = fill or None
        直した.append("塗り")
    if not 直した:
        return "何も指定されていません(bold / number_format などを渡してください)"
    return f"{a1} の {'・'.join(直した)} を直しました"


@mcp.tool()
def autofit(a1: str | None = None, sheet: str | None = None) -> str:
    """列の幅を中身に合わせる。

    `a1` を省くと**そのシート全体**。範囲で絞るなら `A1:C9` のように
    セルまで書く(`A:C` のような列だけの指定は受けない)。
    """
    sh = _sheet(sheet)
    if a1:
        sh[a1].autofit()
        return f"{a1} の幅を合わせました"
    sh.autofit()
    return f"{sh.name} 全体の幅を合わせました"


@mcp.tool()
def save(path: str | None = None) -> str:
    """ブックを保存する。`path` を渡すとその名前で書き出す。

    **人が保存を頼んだときだけ呼ぶ。** 勝手に上書きしない。
    """
    wb = _book()
    wb.save(path) if path else wb.save()
    return f"保存しました: {path or wb.fullname or wb.name}"


# ---- 文書の道具(C-2。2026-08-21)-------------------------------------------
#
# ここまでは表だけでした。文章の受け口(text / set_text / fields / fill_one /
# to_pdf)は既にあるので、同じ作法で包みます。
#
# **道具の名前は doc_ で始めます。** 表の道具と並ぶので、AI が「いま表の
# 話か文書の話か」を取り違えないようにするためです。
#
# **任意のコードを走らせる道具は置きません**(上の決めと同じ)。


def _doc_call(cmd: str, 宛先: str | None = None, **kw):
    """文書の受け口へ1つ送る。**繋がらない・相手が表なら、そう言う**

    `宛先` は「どのタブに送るか」で、`kw` の `path` とは別です
    (`to_pdf` の `path` は*書き出し先*なので、混ぜると壊れます)。
    """
    from . import OfficeworkError, app_name, call

    if 宛先:
        kw["path"] = os.path.abspath(宛先)
    try:
        return call(app_name("writer"), cmd, **kw)
    except OfficeworkError as e:
        言 = str(e)
        if "知らない動詞" in 言:
            raise RuntimeError(
                "いま前に出ているのは表です。文書のタブを前に出すか、"
                "path で文書のファイルを指してください"
            ) from e
        raise RuntimeError(
            f"officework に繋がりません({言})。officework を起動してから使ってください"
        ) from e


@mcp.tool()
def doc_info(path: str | None = None) -> dict:
    """いま開いている**文書**の様子(径路・書きかけかどうか・何枚目か)。

    **文書を触る前にこれを呼ぶ。** 表が前に出ているとここで分かります。
    """
    r = _doc_call("status", path)
    return {
        "path": r.get("path", ""),
        "dirty": r.get("dirty", False),
        "documents": r.get("docs", 1),
        "at": r.get("doc_at", 0),
        "status": r.get("status", ""),
    }


@mcp.tool()
def doc_text(path: str | None = None) -> str:
    """文書の**本文**を読む(いま見ている1枚ぶん)。

    `path` を渡すと、そのファイルを開いているタブから読みます。
    """
    return _doc_call("text", path).get("text", "")


@mcp.tool()
def doc_set_text(text: str, path: str | None = None) -> str:
    """文書の本文を**丸ごと入れ替える**。

    **消える物があります。** 入れ替えるのは本文で、書式やヘッダーは
    テンプレートの側に残ります。書き直す前に `doc_text` で今の中身を
    読んでください。書いた跡は officework の Ctrl+Z で戻せます。
    """
    _doc_call("set_text", path, text=text)
    return f"本文を入れ替えました({len(text)} 字)"


@mcp.tool()
def doc_fields(path: str | None = None) -> list:
    """文書に埋め込んだ**記入欄**の名前を並べる(Word の入力コントロール)。

    本文に書いた `{{名前}}` は別の仕組みです — そちらは
    `doc_merge_fields` で見てください。
    """
    return _doc_call("fields", path).get("fields", [])


@mcp.tool()
def doc_merge_fields(path: str | None = None) -> list:
    """本文の**差し込みの穴**(`{{名前}}`)の名前を並べる。

    まだ埋まっていない穴だけが出ます。埋めるのは `doc_fill` です。

    表の中で行を増やす穴は、**列ごとではなく群の名前**で出ます
    (`明細.品名` ではなく `明細`)。群は行の並びを渡す仕掛けなので、
    いまの `doc_fill`(1つの名前に1つの字)では埋められません。
    """
    return _doc_call("merge_fields", path).get("merge_fields", [])


@mcp.tool()
def doc_fill(values: dict, path: str | None = None) -> str:
    """**差し込みの穴**(`{{名前}}`)に値を入れる。`values` は
    `{"穴の名前": "入れる字"}`。

    その名前の穴が無ければ入れません。**入らなかった名前と、まだ空いて
    いる穴をここで返します** — 黙って落としません。名前は
    `doc_merge_fields` で先に確かめられます。

    表の中で行を増やす群(明細など)はここでは埋められません。
    行の並びを渡す形が要るので、その道具はまだありません。

    名前の付いた記入欄(`doc_fields` で出る物)も、同じ辞書で埋められます。
    名前が穴でなく記入欄なら、記入欄に入れます。
    """
    if not values:
        return "入れる値がありません"
    # **先に穴の名前を見る。** `fill_one` の返す `unknown` は
    # *渡さなかった残りの穴*で、渡した名前が入ったかどうかではありません
    ある = set(_doc_call("merge_fields", path).get("merge_fields", []))
    知らない = [str(n) for n in values if str(n) not in ある]
    # 穴に無い名前は、記入欄(w:sdt)の名前かもしれない
    記入欄 = set(_doc_call("fields", path).get("fields", [])) if 知らない else set()
    欄の名前 = [n for n in 知らない if n in 記入欄]
    知らない = [n for n in 知らない if n not in 記入欄]
    入った = 0
    for 名, 値 in values.items():
        if str(名) in 知らない:
            continue
        if str(名) in 欄の名前:
            _doc_call("fill_field", path, name=str(名), value=str(値))
        else:
            _doc_call("fill_one", path, name=str(名), value=str(値))
        入った += 1
    残り = _doc_call("merge_fields", path).get("merge_fields", [])
    文 = f"{入った} 件を入れました"
    if 欄の名前:
        文 += f"(記入欄: {'・'.join(欄の名前)})"
    if 知らない:
        文 += f"。その名前の穴はありません: {'・'.join(知らない)}"
    if 残り:
        文 += f"。まだ空いている穴: {'・'.join(残り)}"
    return 文


@mcp.tool()
def doc_to_pdf(path: str) -> str:
    """いま前に出ている文書を PDF で書き出す。`path` は**書き出し先**。

    **宛先の指定ではありません** — 書き出すのは前に出ているタブです
    (`save` と同じ作法)。
    """
    _doc_call("to_pdf", None, path=os.path.abspath(path))
    return f"PDF を書きました: {os.path.abspath(path)}"


def main() -> None:
    """`officework-mcp` の入口(標準入出力で MCP を話す)"""
    mcp.run()


if __name__ == "__main__":
    main()
