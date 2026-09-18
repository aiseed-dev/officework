# -*- coding: utf-8 -*-
"""aiseed office の紹介ページ(office.aiseed.dev)。Flet で作っています。

    pip install flet
    flet run --web --port 8550 web/main.py     # 手元で見る
    詳しくは web/README.ja.adoc

載せるのは動いている物だけです(手引きと同じ決め)。
"""
import flet as ft

RELEASES = "https://github.com/aiseed-dev/officework/releases/latest"
GITHUB = "https://github.com/aiseed-dev/officework"
DOCS = "https://github.com/aiseed-dev/officework/tree/main/docs/ja"
PYPI = "https://pypi.org/project/officework/"
ISSUES = "https://github.com/aiseed-dev/officework/issues"
FLET = "https://flet.dev"
VERSION = "0.1.0-alpha.3"

INK = ft.Colors.BLUE_GREY_900
SUB = ft.Colors.BLUE_GREY_700
PAPER = ft.Colors.WHITE
BG = ft.Colors.BLUE_GREY_50


def h1(s):
    return ft.Text(s, size=40, weight=ft.FontWeight.BOLD, color=INK)


def h2(s):
    return ft.Text(s, size=26, weight=ft.FontWeight.BOLD, color=INK)


def p(s, size=17):
    return ft.Text(s, size=size, color=SUB)


def section(title, *body):
    return ft.Container(
        content=ft.Column([h2(title), *body], spacing=14),
        padding=ft.Padding.symmetric(vertical=28, horizontal=24),
        bgcolor=PAPER,
        border_radius=12,
        margin=ft.Margin.only(bottom=24),
    )


def shot(src, caption):
    return ft.Column(
        [
            ft.Image(src=src, fit=ft.BoxFit.CONTAIN, border_radius=6),
            ft.Text(caption, size=15, color=SUB, text_align=ft.TextAlign.CENTER),
        ],
        col={"xs": 12, "md": 6},
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        spacing=8,
    )


def main(page: ft.Page):
    page.title = "aiseed office — Word と Excel のファイルを、同じに開いて刷る"
    # Flutter on the web has no Japanese font of its own and borrows glyphs
    # from Google's Noto per character, so 、 and 。 could come out as the
    # centred Traditional Chinese forms. A Japanese font of our own
    # (Noto Sans JP, OFL, subset to kana, JIS level 1 and this page's text)
    # keeps every glyph Japanese
    page.fonts = {"Noto Sans JP": "fonts/NotoSansJP-Regular.ttf"}
    page.theme = ft.Theme(font_family="Noto Sans JP")
    page.bgcolor = BG
    page.scroll = ft.ScrollMode.AUTO
    page.padding = 0

    hero = ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [ft.Image(src="logo.svg", width=64, height=64), h1("aiseed office")],
                    spacing=16,
                    vertical_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                p("受け取った Word と Excel のファイルを、Word や Excel と同じに開いて、刷ります。", 22),
                p("役所の様式、取引先の帳票、社内のテンプレート。相手が Word や Excel で作った"
                  "ファイルを、同じ頁数、同じ折れ方、同じ列幅で開きます。",),
                ft.Row(
                    [
                        ft.FilledButton(content="ダウンロード(Mac・Windows・Linux)", url=RELEASES),
                        ft.TextButton(content="GitHub", url=GITHUB),
                        ft.TextButton(content="手引き", url=DOCS),
                    ],
                    wrap=True,
                    spacing=12,
                ),
                p(f"いまの版は {VERSION} です。試験公開の段階で、壊れている所は GitHub の Issues に書いてください。", 14),
            ],
            spacing=16,
        ),
        padding=ft.Padding.symmetric(vertical=48, horizontal=24),
    )

    compare = section(
        "同じファイルを両方で開けば、誰でも確かめられます",
        p("名古屋市立大学が公開している「物品借用願」(docx)を、Word と aiseed office で開いた物です。"
          "表の列幅、全角スペースの送り、行の折れ方まで同じになるように作っています。"),
        ft.ResponsiveRow(
            [shot("word.png", "Word で開いた"), shot("office.png", "aiseed office で開いた")],
            spacing=16,
            run_spacing=16,
        ),
        p("役所が公開している様式 288 枚で、Word の PDF と頁数を比べています。"
          "2026 年 9 月 19 日の時点で 246 枚が一致しています。残りも 1 枚ずつ原因を調べて直しています。", 15),
    )

    what = section(
        "できること",
        ft.Markdown(
            "\n".join([
                "* **文書(docx)**: 開く、直す、刷る(PDF)。縦書き、ルビ、均等割り付け、表の中の表、脚注、目次。",
                "* **表(xlsx)**: 開く、計算する(関数 400 あまり)、刷る(PDF)。列の幅、行の高さ、拡大縮小、用紙は Excel と同じです。",
                "* **Python から**: `pip install officework` で、同じエンジンを Python から使えます。docx と xlsx の読み書きと PDF 化ができます。",
                "* **マクロ**: Python で書きます。ファイルの中に実行コードは入れません。",
            ]),
            selectable=True,
            auto_follow_links=True,
        ),
        ft.Row([ft.TextButton(content="PyPI の officework", url=PYPI)]),
    )

    agent = section(
        "見積書と請求書を、日本語と英語で作る AI エージェント",
        p("取引先の名前と品目を伝えると、見積書と請求書を日本語と英語の両方で作り、Word と Excel の"
          "ファイルと PDF にする AI エージェントを作っています。ファイルの読み書きは aiseed office のエンジン、"
          "翻訳は Gemini です。2026 年 10 月のハッカソンに合わせて出します。"),
    )

    support = section(
        "企業向けの有料サポート",
        p("会社で使うときの困りごとを、有料で引き受けます。"),
        ft.Markdown(
            "\n".join([
                "* **様式が合わないときに直します。** お使いの様式が Word や Excel と違って見えるときは、そのファイルを元に原因を調べて直します。",
                "* **優先して答えます。** 不具合の報告と質問に、順番を待たずに答えます。",
                "* **導入を手伝います。** 社内の配布、Python のマクロの書き方、既存の帳票の移し方。",
            ]),
            selectable=True,
        ),
        ft.Row([ft.FilledButton(content="問い合わせ(GitHub の Issues)", url=ISSUES)]),
        p("メールの窓口は準備中です。", 14),
    )

    footer = ft.Container(
        content=ft.Column(
            [
                ft.Row(
                    [
                        ft.TextButton(content="GitHub", url=GITHUB),
                        ft.TextButton(content="手引き", url=DOCS),
                        ft.TextButton(content="Issues", url=ISSUES),
                    ],
                    wrap=True,
                ),
                ft.Row(
                    [p("このページは Python の UI の仕組み ", 14),
                     ft.TextButton(content="Flet", url=FLET),
                     p(" で作っています。", 14)],
                    spacing=0,
                    vertical_alignment=ft.CrossAxisAlignment.CENTER,
                ),
                p("© aiseed", 14),
            ],
            spacing=6,
        ),
        padding=ft.Padding.symmetric(vertical=24, horizontal=24),
    )

    body = ft.Column([hero, compare, what, agent, support, footer], spacing=0, width=1040)
    page.add(ft.Row([body], alignment=ft.MainAxisAlignment.CENTER))


if __name__ == "__main__":
    ft.run(main, assets_dir="assets")
