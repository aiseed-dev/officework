# 月に一度の便りを、客の台帳から一人ずつ作る。中身はすべて架空。
#
#   pip install officework
#   python3 sample/在庫から配り物.py   # 先に在庫の正本を作る
#   python3 sample/月の便り.py
#
# **月1のメールがいい**(発注者 2026-08-15)。チャットの場は作らない —
# メールと電話は既にあり、二重に持つと客が困る。作るのは**便りの下書き**。
#
# **送らない。** ここが作るのは下書き(.eml)まで。送るのは人。
# 数百通を勝手に送る道具にはしない — 一度出た物は取り消せない。
#
# ## 法の縛りを道具で守る(特定電子メール法)
#
# 総務省・消費者庁のガイドラインの要点を、この台本はこう受ける:
#
# - **同意の無い人には作らない。** 過去に買った人は「取引関係にある者」で
#   オプトインの例外に当たりうるが、案内が主目的なら規制が掛かる。だから
#   台帳の「案内可」が ○ の人だけを対象にし、**外した人と理由を必ず言う**
# - **断った人には二度と作らない。** 台帳の「断った日」が入っていれば外す
# - **差出人の氏名・住所と、受信拒否の連絡先を必ず末尾に入れる**(表示義務)。
#   ここは台本が機械的に付けるので、書き忘れが起きない
import datetime
import email.message
import email.utils
import pathlib

from officework import sheet

ここ = pathlib.Path(__file__).resolve().parent
正本 = ここ / "種の在庫.xlsx"
客の台帳 = ここ / "客の台帳.xlsx"
下書き置き場 = ここ / "便りの下書き"

# 差出人(表示義務。**架空**)
差出人 = {
    "名前": "たねの畑",
    "住所": "〒000-0000 どこかの県どこかの市1-2-3",
    "メール": "tane@example.invalid",
    "電話": "000-000-0000",
}

台帳の見出し = ["番号", "お名前", "メール", "ご住所", "最後の注文", "案内可", "断った日", "覚え書き"]

見本の客 = [
    ("山田 花子", "hanako@example.invalid", "〒000-0000 どこかの県どこかの市1-2-3",
     "2026-08-15", "○", "", "青しそと聖護院かぶ"),
    ("鈴木 一郎", "ichiro@example.invalid", "〒111-1111 となりの県となりの市4-5-6",
     "2026-07-02", "○", "", "豆をまとめて"),
    ("佐藤 みどり", "midori@example.invalid", "〒222-2222 遠くの県遠くの市7-8-9",
     "2026-05-20", "", "", "**同意をもらっていない** — 作らない"),
    ("高橋 太郎", "taro@example.invalid", "〒333-3333 むこうの県むこうの市1-1-1",
     "2026-03-11", "○", "2026-06-30", "**断られた** — 作らない"),
]


def 客の台帳を作る():
    """**実物の代わり。** 台帳があれば要らない。
    「案内可」と「断った日」の欄が肝 — ここが無いと法を守れない"""
    if 客の台帳.exists():
        return 0
    b = sheet.Book()
    ws = b.active
    ws.title = "客の台帳"
    for c, 名 in enumerate(台帳の見出し, start=1):
        cell = ws.cell(row=1, column=c)
        cell.value = 名
        cell.font = sheet.Font(bold=True)
    for i, 行 in enumerate(見本の客, start=2):
        ws.cell(row=i, column=1).value = f"{i - 1:03d}"
        for c, v in enumerate(行, start=2):
            ws.cell(row=i, column=c).value = v
    for col, w in (("A", 8), ("B", 16), ("C", 28), ("D", 34),
                   ("E", 14), ("F", 10), ("G", 12), ("H", 30)):
        ws.column_dimensions[col].width = w
    ws.freeze_panes = "A2"
    b.save(客の台帳)
    return len(見本の客)


def 客を読む():
    """(送る人, 外した人と理由)。**外した人を黙って落とさない**"""
    b = sheet.Book.open(客の台帳)
    ws = b[b.sheet_names[0]]
    送る, 外した = [], []
    for r in ws.values()[1:]:
        番号, 名前, メール, 住所, 最後, 案内可, 断った日, _ = (list(r) + [None] * 8)[:8]
        if not 名前:
            continue
        if 断った日:
            外した.append(f"{名前}: {断った日} に断られています")
            continue
        if str(案内可 or "").strip() not in ("○", "◯", "はい", "yes", "y"):
            外した.append(f"{名前}: 案内を送る同意がありません")
            continue
        if not メール:
            外した.append(f"{名前}: メールの宛先がありません")
            continue
        送る.append((番号, 名前, メール, 住所, 最後))
    return 送る, 外した


def 今月の品():
    """在庫のある物だけ。**無い物を案内しない**(問い合わせを無駄にする)"""
    b = sheet.Book.open(正本)
    ws = b[b.sheet_names[0]]
    出せる = []
    for r in ws.values()[1:]:
        番号, 分類, 品名, 単価, 数, _ = (list(r) + [None] * 6)[:6]
        if 数 and 数 > 0:
            出せる.append((番号, 分類, 品名, 単価, 数))
    return 出せる


def 便りの本文(名前, 品, 一言, 月):
    行 = [f"{名前} 様", "", 一言, "", f"■ {月} にお出しできる種", ""]
    分類ごと = {}
    for 番号, 分類, 品名, 単価, 数 in 品:
        分類ごと.setdefault(分類, []).append((番号, 品名, 単価, 数))
    for 分類, 品目 in 分類ごと.items():
        行.append(f"【{分類}】")
        for 番号, 品名, 単価, 数 in 品目:
            残り = "(残りわずか)" if 数 <= 5 else ""
            行.append(f"  {番号} {品名} … {単価:,}円 {残り}".rstrip())
        行.append("")
    行 += [
        "ご注文は、この返信か、お電話でどうぞ。",
        "",
        "-" * 40,
        f"{差出人['名前']}",
        f"{差出人['住所']}",
        f"電話 {差出人['電話']} / メール {差出人['メール']}",
        "",
        "※ 今後この案内が要らない場合は、このメールに「不要」とだけ",
        f"   返信してください({差出人['メール']})。以後お送りしません。",
    ]
    return "\n".join(行)


def 下書きを作る(送る, 品, 月, 一言):
    """**.eml で置く。** メールの道具でそのまま開いて、直してから送れる。
    ここでは送らない — 一度出た物は取り消せない"""
    下書き置き場.mkdir(exist_ok=True)
    for f in 下書き置き場.glob("*.eml"):
        f.unlink()
    for 番号, 名前, メール, _, _ in 送る:
        m = email.message.EmailMessage()
        m["From"] = email.utils.formataddr((差出人["名前"], 差出人["メール"]))
        m["To"] = email.utils.formataddr((名前, メール))
        m["Subject"] = f"{月}のたねのお便り"
        m["Date"] = email.utils.format_datetime(
            datetime.datetime(2026, 8, 15, 9, 0))
        m.set_content(便りの本文(名前, 品, 一言, 月))
        (下書き置き場 / f"{番号}_{名前}.eml").write_bytes(bytes(m))
    return len(送る)


if __name__ == "__main__":
    n = 客の台帳を作る()
    if n:
        print(f"客の台帳を作りました: {客の台帳.name}({n} 件)")

    品 = 今月の品()
    送る, 外した = 客を読む()
    月 = "8月"
    一言 = ("暑さが続きます。しそが盛りで、豆の莢がふくらんできました。\n"
            "今年は雨が少なく、かぶは小ぶりですが味は濃いです。")

    件数 = 下書きを作る(送る, 品, 月, 一言)
    print(f"今月お出しできる種: {len(品)} 品目")
    print(f"下書き: {下書き置き場.name}/ に {件数} 通")
    for 言い分 in 外した:
        print(f"  作りませんでした — {言い分}")
    print()
    print("**送っていません。** 下書きを読んで、直してから自分で送ってください。")
    print("末尾の差出人と受信拒否の案内は台本が付けています(表示義務)。")
