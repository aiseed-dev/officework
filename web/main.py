# -*- coding: utf-8 -*-
"""aiseed office の紹介ページ(office.aiseed.dev)。Flet で作っています。

    pip install flet
    flet run --web --port 8550 web/main.py     # 手元で見る
    詳しくは web/README.ja.adoc

載せるのは動いている物だけです(手引きと同じ決め)。
"""
import asyncio

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
    return ft.Text(s, size=40, weight=ft.FontWeight.BOLD, color=INK, selectable=True)


def h2(s):
    return ft.Text(s, size=26, weight=ft.FontWeight.BOLD, color=INK, selectable=True)


def p(s, size=17):
    # Every line of text on the page can be selected and copied (2026-09-19,
    # the owner). Flutter draws text itself, so this has to be asked for
    return ft.Text(s, size=size, color=SUB, selectable=True)


def md(text):
    # Markdown's own default text is too light against the card; give the
    # paragraphs and bullets the same colour and size as p()
    style = ft.TextStyle(color=SUB, size=17)
    return ft.Markdown(
        text,
        selectable=True,
        auto_follow_links=True,
        md_style_sheet=ft.MarkdownStyleSheet(p_text_style=style, list_bullet_text_style=style),
    )


def section(title, *body):
    return ft.Container(
        content=ft.Column([h2(title), *body], spacing=14),
        padding=ft.Padding.symmetric(vertical=28, horizontal=24),
        bgcolor=PAPER,
        border_radius=12,
        margin=ft.Margin.only(bottom=24),
    )


def shot(src, caption, page):
    async def copy(_):
        # assets/index.html watches the page title and copies the image
        # while it reads "copy:<file>"; the title goes back right after
        title = page.title
        page.title = f"copy:{src}"
        page.update()
        # two updates in one handler are sent as one, so the page would
        # never see the marker; let the first one reach the browser
        await asyncio.sleep(0.3)
        page.title = title
        page.update()
        page.show_dialog(ft.SnackBar(ft.Text("画像をクリップボードに入れました。")))

    return ft.Column(
        [
            ft.Image(src=src, fit=ft.BoxFit.CONTAIN, border_radius=6),
            ft.Row(
                [
                    ft.Text(caption, size=15, color=SUB, selectable=True),
                    ft.TextButton(content="画像をコピー", on_click=copy),
                ],
                alignment=ft.MainAxisAlignment.CENTER,
                spacing=12,
            ),
        ],
        col={"xs": 12, "md": 6},
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        spacing=8,
    )


def main(page: ft.Page):
    page.title = "aiseed office — Word と Excel のファイルを、同じに開いて刷る"
    # Japanese glyphs: see assets/index.html. Flutter fills in CJK characters
    # from Google's Noto by the browser language, so the page declares itself
    # Japanese there instead of bundling a font
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

    why = section(
        "なぜ作ったか",
        p("AI が必要でない場所で使うための Office です。", 20),
        p("文書は、AI が会話から作れるようになりました。しかし、毎日の仕事の大半に AI は要りません。"
          "受け取った docx と xlsx を、Word と Excel と同じに開いて、処理して、様式で返して、刷る。"
          "これは決まった手順で、コードだけで回ります。"),
        p("そこには、有料の AI も、Microsoft 365 も、Windows も要りません。機密の様式は手元から出ません。"
          "AI が要るときだけ、自分で選んだ AI を繋ぎます。"),
        p("Debian の上で、現場が自分の様式を自分の道具で扱うための Office として作っています。"),
    )

    goal = section(
        "持つようにする機能",
        p("これは目指す物です。いまできることは、下の節にあります。", 15),
        md("\n".join([
            "* **受け取った docx と xlsx を、Word と Excel と同じ頁数、同じ折れ方、同じ列幅で開いて、刷る。** これが第一です。",
            "* **様式の流れを Python で回す。** 様式に書く → 受け取る → 処理する → 様式で返す。",
            "* **機密を外に出さない。** ファイルは手元にあり、繋ぐ AI は自分で選べ、毎日の運用は AI 無しでコードだけで回ります。",
            "* **Debian で動く。** Windows を消して、Windows でしか動く物は仮想環境の中に置きます。",
            "* **印刷。** PDF と、プリンターへ。",
            "* **有料の AI が要らない現場のための物。** AI を使うのは、作る人が開発と保守をするときだけです。",
        ])),
    )

    how = section(
        "作り方",
        p("Office のソフトを作るのは、もう難しいことではありません。理屈はこうです。"),
        md("\n".join([
            "* **AI は、すでに docx と xlsx を作れます。** 2026 年 9 月 16 日(米国時間)、Anthropic は、会話から文書とスライドを作り、PowerPoint と PDF で落とせる Claude Docs と Claude Slides を出しました。AI はファイルの形式を正しく書けます。",
            "* **形式は公開されています。** docx と xlsx の中身(OOXML)は仕様が公開されていて、書ける物は読めます。",
            "* **答え合わせができます。** 同じファイルを Word と両方で開いて並べれば、合っているかは誰でも分かります。ルールが決まっていて、答え合わせが機械でできる物は、AI が得意になります。囲碁と同じです。",
            "* **だから作り方は 1 つです。** 人が決めて(何を同じにするか、その順番)、AI が書いて、動かして比べて、違った所を直す。この繰り返しです。難しい所はなく、あるのは回数だけです。",
            "* **要る物は、回線と月 100 ドルです。** Claude の Max は月 100 ドルからです。",
        ])),
    )

    compare = section(
        "同じファイルを両方で開けば、誰でも確かめられます",
        p("名古屋市立大学が公開している「物品借用願」(docx)を、Word と aiseed office で開いた物です。"
          "表の列幅、全角スペースの送り、行の折れ方まで同じになるように作っています。"),
        ft.ResponsiveRow(
            [shot("word.png", "Word で開いた", page),
             shot("office.png", "aiseed office で開いた", page)],
            spacing=16,
            run_spacing=16,
        ),
        ft.Row(
            [
                ft.FilledButton(content="この様式(docx)をダウンロード", url="buppin202411.docx"),
                p("名古屋市立大学の物品借用願です。お手元の Word と aiseed office で開いて、並べてみてください。", 14),
            ],
            wrap=True,
            spacing=12,
            vertical_alignment=ft.CrossAxisAlignment.CENTER,
        ),
        p("役所が公開している様式 288 枚で、Word の PDF と頁数を比べています。"
          "2026 年 9 月 19 日の時点で 246 枚が一致しています。残りも 1 枚ずつ原因を調べて直しています。", 15),
    )

    what = section(
        "いまできること",
        md("\n".join([
            "* **文書(docx)**: 開く、直す、刷る(PDF)。縦書き、ルビ、均等割り付け、表の中の表、脚注、目次。",
            "* **表(xlsx)**: 開く、計算する(関数 400 あまり)、刷る(PDF)。列の幅、行の高さ、拡大縮小、用紙は Excel と同じです。",
            "* **Python から**: `pip install officework` で、同じエンジンを Python から使えます。docx と xlsx の読み書きと PDF 化ができます。",
            "* **マクロ**: Python で書きます。ファイルの中に実行コードは入れません。",
        ])),
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
        md("\n".join([
            "* **様式が合わないときに直します。** お使いの様式が Word や Excel と違って見えるときは、そのファイルを元に原因を調べて直します。",
            "* **優先して答えます。** 不具合の報告と質問に、順番を待たずに答えます。",
            "* **導入を手伝います。** 社内の配布、Python のマクロの書き方、既存の帳票の移し方。",
        ])),
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

    body = ft.Column([hero, why, goal, how, compare, what, agent, support, footer], spacing=0)

    def fit(width):
        # at most 1040 wide, never wider than the browser (phones)
        body.width = min(1040, width or 1040)

    def on_resize(e):
        fit(e.width)
        page.update()

    page.on_resize = on_resize
    fit(page.width)
    page.add(ft.Row([body], alignment=ft.MainAxisAlignment.CENTER))


if __name__ == "__main__":
    ft.run(main, assets_dir="assets")
