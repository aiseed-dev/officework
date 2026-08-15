"""officework を **MCP の道具**として差し出す(2026-08-15)。

**向きが逆になる。** これまでは officework が AI を呼んでいた。ここでは
**AI の側**(利用者が動かしている Claude Code / Claude Desktop / その他の
MCP の客)が officework を道具として使う。

    「いま開いている表の B 列を合計して、下に置いて」

と利用者が自分の AI に言えば、その AI がここの道具を呼んで calc を操る。

# なぜこの形か

1. **契約の話が消える。** 枠を使うのは利用者と、その人が選んだ AI の
   製品との間。officework は「ログインと枠を提供する」場面を持たない
2. **どの客からでも使える。** Claude Code でも Claude Desktop でも、
   MCP を話す物なら何でも
3. **薄い。** 芯は既にある — calc は AF_UNIX の受け口(ops の 52 命令)を
   持ち、`officework.calc` がその橋。ここはそれを道具の名前で包むだけ

# 使い方

    pip install "officework[mcp]"

客の設定に足す(Claude Code なら `claude mcp add`、Claude Desktop なら
設定ファイル):

    officework-mcp

**calc が動いている必要がある。** 動いていなければ、そう言って断る
(黙って空を返さない)。

# 何を渡していないか

- **任意のコードを走らせる道具は置かない。** 「Python を実行」を1つ
  置けば何でもできてしまい、利用者が許した範囲を越える
- 触るのは**いま開いているブック**だけ。ファイルを開く道具は置いていない
  — 何を開くかは人が calc の側で決める
- 保存は人が頼んだときだけ(`save`)。勝手に上書きしない
"""

from __future__ import annotations

try:
    from mcp.server.fastmcp import FastMCP
except ModuleNotFoundError as e:  # pragma: no cover - 入れていない人向け
    raise SystemExit(
        "MCP の口には mcp が要ります: pip install \"officework[mcp]\""
    ) from e

from . import calc as xw

mcp = FastMCP("officework")


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


def main() -> None:
    """`officework-mcp` の入口(標準入出力で MCP を話す)"""
    mcp.run()


if __name__ == "__main__":
    main()
