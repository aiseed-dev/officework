#!/usr/bin/env python3
"""ribbon.rs(正)から、別ロケールのリボン表を起こす。

gen_ribbon.py(テンプレートから ja の表を起こす)と役割が違う:
こちらは **いまの face/src/ribbon.rs を構造の正** とし、語だけを
Euro-Office のロケール(vendor/web-apps の ja.json → <locale>.json の対訳)で
置き換える。手で足したボタン(AI タブなど本家に無いもの)は OVERRIDES 表で
訳す。**訳が見つからない語があれば止まる**(黙って日本語のまま出さない)。

    python3 ui/gen_ribbon_locale.py en > face/src/ribbon_en.rs

id・並び・ready・icon は ja と同一になる(試験 ribbon.rs 側で保証)。
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from ribbon_parse import tables_or_die  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent / "vendor/web-apps/apps"
# **リボンの表は face(gpui を持たない層)へ移った**(2026-08-15)。
# ここを直し忘れると、13言語の生成が丸ごと止まる
RIBBON = Path(__file__).resolve().parent.parent / "face/src/ribbon.rs"

# 本家に無い・こちらで足した語の対訳。ここに無い未解決語が出たら
# このスクリプトは止まる — その語をここに足してから出し直す
OVERRIDES = {
    "en": {
        # **セルの中の文字を回すボタン**(2026-08-21)。本家の日本語は
        # ページの向きと同じ「印刷の向き」で、押すまで区別できませんでした。
        # 日本語を Excel の「方向」にしたので、訳は本家の
        # SSE.Views.Toolbar.tipTextOrientation から取ります
        "方向": "Orientation",
        # **セルの書式設定**(2026-08-21)。日本語は Excel の言葉にしたので
        # 本家の日本語(「セルをフォーマットする」)と字面が合いません。
        # 訳は本家の SSE.Views.DocumentHolder.txtCellFormat から取ります
        "セルの書式設定": "Format cells",
        "上下中央揃え": "Align middle",
        # 式から呼べる Python の関数の一覧(2026-08-16。本家に無い)
        "Python の関数": "Python functions",
        # リボンに出るマクロの一覧(2026-08-16。本家に無い)
        "リボンのマクロ": "Ribbon macros",
        "書式のコピー": "Format painter",
        "スタイル": "Style",
        "フィールドリスト": "Field list",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Bigger UI text",
        "画面の文字を小さく": "Smaller UI text",
        # **入力規則に合っていない値を洗い出すボタン**(2026-08-21 の D群)。
        # 本家にこの機能そのものがないので、語もこちらで用意します
        "無効データのマーク": "Circle invalid data",
        "分割": "Split",
        # タブ
        "AI": "AI",
        # ファイル
        "印刷": "Print",
        "印刷レイアウト": "Print layout",
        # AI タブ(こちらの設計。calc-manual.md の英語版と同じ語)
        "宛先": "Destination",
        "要約": "Summarize",
        "書き直す": "Rewrite",
        "敬語にする": "Politer",
        "やさしく": "Plainer",
        "翻訳": "Translate",
        "ふりがな": "Furigana",
        "続きを書く": "Continue",
        "表にする": "To table",
        "頼む": "Ask",
        # writer 独自
        "ルビ": "Ruby",
        "縦書き": "Vertical text",
        "テキスト方向": "Text direction",
        "均等割付": "Distributed",
        "図表番号の挿入": "Insert caption",
        "URL を開く": "Open URL",
        "洋子さんの索引": "Index",
        "青空文庫の注記": "Aozora notes",
        "でんでん記法": "Denden markup",
        "履歴の記録": "Track changes",
        "変更履歴の表示": "Show changes",
        "校正": "Proofread",
        "文字数": "Character count",
        "スペルチェック": "Spell check",
        "類語辞典": "Thesaurus",
        "誤変換": "Misconversion",
        "表記ゆれ": "Inconsistency",
        # calc 独自
        "小計": "Subtotal",
        "計算方法": "Calculation",
        "右横書き": "Right-to-left text",
        "シートの方向": "Sheet direction",
        "Python": "Python",
        "チェックボックス": "Checkbox",
        "外部リンク": "External links",
        "推奨チャート": "Recommended chart",
        # 共同編集・保護(writer/calc 共通の言い換え)
        "共同編集モード": "Co-editing mode",
        "バージョン履歴": "Version history",
        "チャット": "Chat",
        "保護する": "Protect",
        "暗号化する": "Encrypt",
        "デジタル署名を追加": "Add digital signature",
        "マクロ": "Macros",
        "プラグインの管理": "Manage plugins",
        # 本家の語と言い回しが少し違うもの(Word/Excel の標準語で)
        "0を表示する": "Show zeros",
        "100%に拡大する": "Zoom to 100%",
        "インターフェイステーマ": "Interface theme",
        "ウォッチウィンドウ": "Watch window",
        "オートSUM": "AutoSum",
        "操作を記録": "Record actions",
        "コメントを削除": "Delete comment",
        "ソルバー": "Solver",
        "テキストからデータ": "Text to data",
        "トレース矢印の削除": "Remove arrows",
        "フィル": "Fill",
        "フィルターを解除": "Clear filter",
        "マクロを書く": "Write macro",
        "区切り位置": "Text to columns",
        "図表番号": "Caption",
        "図表目次": "Table of figures",
        "図表目次の更新": "Update table of figures",
        "外部リンク(値で取り込む)": "External links (import as values)",
        "数学/三角": "Math & Trig",
        "数式の表示": "Show formulas",
        "文字の向き(右横書き)": "Right-to-left text",
        "文字列操作": "Text",
        "日付/時刻": "Date & Time",
        "最近使った関数": "Recently used",
        "枠線も印刷": "Print gridlines",
        "目次の更新": "Update table of contents",
        "縞模様の列": "Banded columns",
        "見出しも印刷": "Print headings",
        "詳細の非表示": "Hide detail",
        "重複の削除": "Remove duplicates",
        "関数の挿入": "Insert function",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV format",
        "セルのロック": "Cell lock",
        "データテーブル": "Data table",
        "フラッシュフィル": "Fill by example",
        "一覧": "List",
        "名前を貼り付け": "Paste name",
        "復旧": "Recover",
        "折り返して全体を表示する": "Wrap text",
        "控えの間隔": "Backup interval",
        "新しい .py": "New .py",
        "範囲を足す": "Add to area",
        "紙に収める": "Fit to paper",
        "紙の切れ目": "Page breaks",
        "置き場を開く": "Open folder",
        "計算の種類": "Show values as",
        "許可する操作": "Allowed actions",
        "読み取り専用を勧める": "Suggest read-only",
    },
    # vendor のロケールに無い語の穴埋め(gen_lang.py が材料の訳と併用する)
    "zh-tw": {
        # 本家の台湾語は「尋找和引用」— **引用は大陸の言い方**。
        # こちらの台湾語の材料は 參照 26 回・引用 0 回で、台湾の Excel も
        # 「查閱與參照」(2026-08-11、分類の耳を訳した下請けが数えて指摘)
        "検索/行列": "查閱與參照",
        "ページ数": "頁數",
        "表のデザイン": "表格設計",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "it": {
        "フィルタのボタン": "Pulsante filtro",
        "ヘッダー行": "Riga di intestazione",
        "合計行": "Riga totale",
        "最後の列": "Ultima colonna",
        "範囲に変換する": "Converti in intervallo",
        "表のデザイン": "Struttura tabella",
        "テーブルのサイズ変更": "Ridimensiona tabella",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "tr": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "罫線": "Sınırlar",
        # **本家のトルコ語が誤訳**(2026-08-21)。Insert chart に
        # Tablo ekle(表を挿入)が入っていて、「表の挿入」と同じ
        # ラベルになっていました。tablo は表、grafik がグラフです。
        # 語は LibreOffice の公式訳(Insert Chart)から取りました
        "グラフを挿入": "Grafik ekle",
        "範囲を保護する": "Aralığı koru",
        "図形を結合": "Şekilleri birleştir",
        "改ページ プレビュー": "Sayfa Sonu Önizlemesi",
        "フィルタのボタン": "Filtre düğmesi",
        "ヘッダー行": "Üst bilgi satırı",
        "ページ数": "Sayfa sayısı",
        "印刷物で次のページを開始する位置に改行を追加する": "Yeni sayfanın başlayacağı yere sayfa sonu ekle",
        "参照元のトレース": "Etkileyenleri izle",
        "参照先のトレース": "Etkilenenleri izle",
        "合計行": "Toplam satırı",
        "推奨チャートを挿入": "Önerilen grafik ekle",
        "最初の列が右側に来るようにシートの方向を切り替える": "Sayfa yönünü ilk sütun sağda olacak şekilde değiştir",
        "最後の列": "Son sütun",
        "範囲に変換する": "Aralığa dönüştür",
        "罫線": "Kenarlıklar",
        "蛍光ペン": "Vurgulayıcı",
        "表のデザイン": "Tablo tasarımı",
        "カンマスタイル": "Virgül stili",
        "ゴールシーク": "Hedef Ara",
        "テーブルのサイズ変更": "Tabloyu yeniden boyutlandır",
        "ファイルからのテキスト": "Dosyadan metin",
        "SmartArtの挿入": "SmartArt ekle",
        "すべて更新": "Tümünü yenile",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "id": {
        "図形を結合": "Gabungkan bentuk",
        "ゴールシーク": "Pencarian Tujuan",
        "テーブルのサイズ変更": "Ubah ukuran tabel",
        "フィルタのボタン": "Tombol filter",
        "ヘッダー行": "Baris header",
        "ページ数": "Jumlah halaman",
        "合計行": "Baris total",
        "最初の列が右側に来るようにシートの方向を切り替える": "Ubah arah lembar agar kolom pertama di kanan",
        "最後の列": "Kolom terakhir",
        "範囲に変換する": "Konversi ke rentang",
        "表のデザイン": "Desain tabel",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "vi": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "罫線": "Đường viền",
        "シートを保護する": "Bảo vệ trang tính",
        "ブックを保護する": "Bảo vệ sổ làm việc",
        "範囲を保護する": "Bảo vệ phạm vi",
        "図形を結合": "Hợp nhất hình dạng",
        "改ページ プレビュー": "Xem trước ngắt trang",
        "SmartArtの挿入": "Chèn SmartArt",
        "すべて更新": "Làm mới tất cả",
        "その他の関数": "Hàm khác",
        "ウィンドウ枠の固定": "Cố định ngăn",
        "カンマスタイル": "Kiểu dấu phẩy",
        "コンボボックス": "Hộp tổ hợp",
        "ゴールシーク": "Tìm mục tiêu",
        "シートの表示": "Hiện trang tính",
        "ステータスバー": "Thanh trạng thái",
        "スパークラインを挿入する": "Chèn biểu đồ thu nhỏ",
        "スライサーを挿入": "Chèn slicer",
        "タイトルを印刷する": "In tiêu đề",
        "ダークモード": "Chế độ tối",
        "ツールバーを常に表示する": "Luôn hiện thanh công cụ",
        "テキストの追加": "Thêm chữ",
        "テキストフィールド": "Trường văn bản",
        "テーブルのサイズ変更": "Đổi cỡ bảng",
        "データの入力規則": "Xác thực dữ liệu",
        "ドロップダウン": "Danh sách thả xuống",
        "ナビゲーション": "Dẫn hướng",
        "ハイフン設定の変更": "Ngắt từ bằng dấu gạch nối",
        "ピボットテーブル": "PivotTable",
        "ピボットテーブルを挿入": "Chèn PivotTable",
        "ファイルからのテキスト": "Văn bản từ tệp",
        "フィルタのボタン": "Nút lọc",
        "フィルター": "Bộ lọc",
        "フォーム": "Biểu mẫu",
        "ブックマーク": "Dấu trang",
        "ヘッダー行": "Hàng tiêu đề",
        "ペン": "Bút",
        "ページ数": "tổng số trang",
        "ページ番号": "số trang",
        "ページ色の変更": "Màu trang",
        "メールアドレス": "Địa chỉ email",
        "ラジオボタン": "Nút radio",
        "ルーラー": "Thước",
        "レポートのレイアウト": "Bố cục báo cáo",
        "印刷物で次のページを開始する位置に改行を追加する": "Chèn ngắt trang tại vị trí bắt đầu trang mới",
        "印刷範囲": "Vùng in",
        "参照元のトレース": "Truy vết ô ảnh hưởng",
        "参照先のトレース": "Truy vết ô phụ thuộc",
        "右パネル": "Bảng bên phải",
        "合計行": "Hàng tổng",
        "大文字小文字を変更": "Đổi chữ hoa/thường",
        "左パネル": "Bảng bên trái",
        "拡大縮小印刷": "Co giãn khi in",
        "推奨チャートを挿入": "Chèn biểu đồ đề xuất",
        "数式バー": "Thanh công thức",
        "斜体": "Nghiêng",
        "更新": "Làm mới",
        "最初の列": "Cột đầu",
        "最初の列が右側に来るようにシートの方向を切り替える": "Đổi hướng trang tính để cột đầu ở bên phải",
        "条件付き書式": "Định dạng có điều kiện",
        "検索/行列": "Tra cứu & tham chiếu",
        "相互参照": "Tham chiếu chéo",
        "空白ページの挿入": "Chèn trang trống",
        "空行": "Dòng trống",
        "総計": "Tổng chung",
        "縞模様の行": "Hàng xen kẽ màu",
        "罫線": "Viền",
        "蛍光ペン": "Bút dạ quang",
        "行番号を表示する": "Hiện số dòng",
        "表のデザイン": "Thiết kế bảng",
        "複合フィールド": "Trường phức hợp",
        "複数ページ": "Nhiều trang",
        "見出し": "Tiêu đề",
        "記号を挿入": "Chèn ký hiệu",
        "論理": "Logic",
        "財務": "Tài chính",
        "透かしを編集する": "Sửa hình mờ",
        "重複データを削除": "Xóa dữ liệu trùng lặp",
        "開く": "Mở",
        "電話番号": "Số điện thoại",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "de": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "es": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "fr": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "pt-br": {
        # ブラジル**だけ**を分ける札(2026-08-11 発注者)
        # 本家のブラジル語そのものが誤っていた3語。ブラジル語としても
        # 誤りなので、欧州版と一緒に直す(2026-08-11):
        #   Projeto da mesa   = 家具の机の設計(table を家具と取った)
        #   Total de linhas   = 行数(「合計の行」ではない)
        #   Faixa de proteção = 保護の帯(命令の動詞が要る所を名詞句に)
        "表のデザイン": "Design da Tabela",
        "合計行": "Linha de Totais",
        "範囲を保護する": "Proteger Intervalo",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "pt": {
        # 表の「罫線」。本家の日本語は「表の枠線」でしたが、セルに引く線
        # なので Excel は「罫線」です。この言語は文章の側の「罫線」から
        # 引けないので、表の側の鍵(tipBorders)を書きます
        "罫線": "Bordas",
        # **本家の欧州ファイル(pt-pt.json)は薄い。** 21 語は訳が無く、
        # 2 語はブラジル語が紛れていた("Estilo de porcentagem"、
        # データのタブが "Data"=日付)。**本家にあることは正しいことでは
        # ない** — 欠けたところは原文と英語から訳し、隣の言語から写さない
        # (2026-08-11。訳語の出どころは docs/sekkei/calc.ja.md)
        "SmartArtの挿入": "Inserir SmartArt",
        "カンマスタイル": "Estilo de vírgula",
        "ゴールシーク": "Atingir objetivo",
        "テーブルのサイズ変更": "Redimensionar tabela",
        "ハイフン設定の変更": "Alterar a hifenização",
        "ファイルからのテキスト": "Texto de um ficheiro",
        "フィルタのボタン": "Botão de filtro",
        "ヘッダー行": "Linha de cabeçalho",
        "ページ数": "Número de páginas",
        "ページ色の変更": "Alterar a cor da página",
        "印刷物で次のページを開始する位置に改行を追加する": "Adicione uma quebra no sítio onde a página seguinte deve começar na cópia impressa",
        "参照元のトレース": "Rastrear Precedentes",
        "合計行": "Linha de totais",
        "図形を結合": "Unir formas",
        "推奨チャートを挿入": "Inserir gráfico recomendado",
        "最初の列が右側に来るようにシートの方向を切り替える": "Inverta a direção da folha para que a primeira coluna fique do lado direito",
        "最後の列": "Última coluna",
        "範囲に変換する": "Converter em intervalo",
        "範囲を保護する": "Proteger intervalo",
        "罫線": "Bordas",
        "表のデザイン": "Estrutura da Tabela",
        "データ": "Dados",
        "パーセントのスタイル": "Estilo de percentagem",
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "ru": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "ko": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
    "zh": {
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
},
}


# 本家の綴りがこちらと違うもの。中身と経緯は正本 ui/locales.py に —
# ポルトガル語は札の意味が逆(向こうの pt.json はブラジル)
import locales
VENDOR_LOCALE = locales.VENDOR


def load(app, loc):
    """本家の対訳を読む。綴りが違えば `VENDOR_LOCALE` で読み替える"""
    want = VENDOR_LOCALE.get(loc, loc)
    for name in (want, want.lower()):
        p = ROOT / app / f"main/locale/{name}.json"
        if p.exists():
            return json.load(open(p, encoding="utf-8"))
    sys.exit(
        f"ロケールの現物が見つかりません: {ROOT / app / f'main/locale/{want}.json'}"
        + (f"(こちらの {loc} を {want} と読み替えた)" if want != loc else "")
    )


def build_map(apps, target):
    """ja の語 → target の語。同じ ja 語に複数候補があれば多数決 →
    短い順 → 辞書順(決定的に選ぶ)"""
    cand: dict[str, Counter] = {}
    for app in apps:
        ja = load(app, "ja")
        tr = load(app, target)
        for k, jv in ja.items():
            tv = tr.get(k)
            if not isinstance(jv, str) or not isinstance(tv, str):
                continue
            if not jv.strip() or not tv.strip():
                continue
            cand.setdefault(jv, Counter())[tv] += 1
    out = {}
    for jv, c in cand.items():
        best = sorted(c.items(), key=lambda kv: (-kv[1], len(kv[0]), kv[0]))[0][0]
        out[jv] = best
    return out


# `ribbon.rs` の読みは tools/ribbon_parse.py に集めた(2026-08-12)。
# ここにあった自前の正規表現は「合致する物を拾う」形で、**書き方が変われば
# 静かに減る**。5つの道具が同じ穴を持っていたので、1枚に寄せた。
#
# あちらは領域を**食べ尽くして、残りが1文字でも出たら落ちる**。読み落としが
# 無い代わりに、表の書き方を変えたら解析器も直すことになる — その取引で正しい。
# **この生成器は特に、読み落とすと生成物からボタンが消える**(黙って)。


# 本家の英語は米国綴りの1種類しかない。こちらの `en` は英国基準と
# 決めた(2026-08-11 発注者「英国基準がいいのでは」)ので、本家から
# 来た語を綴り直す。
#
# **札にだけ掛ける。** はじめ「米国綴りは Center の1語だけ」と数えて
# 上書き表に1行足して済ませたが、それは**大文字で始まる語しか
# 数えていなかった** — 実際には "Font color" のように語中に 7 件あった。
# そして id にも `align-center` があるので、ファイル全体に掛けると
# ボタンの id が変わって配線が切れる。掛ける場所を間違えると、
# 綴りが直る代わりにボタンが死ぬ。
BRITISH = {
    "color": "colour",
    "colors": "colours",
    "center": "centre",
    "centers": "centres",
    "centered": "centred",
    "organizer": "organiser",
    "customize": "customise",
    "customized": "customised",
    "analyze": "analyse",
    "gray": "grey",
}
_BRITISH_RE = re.compile(
    r"\b(" + "|".join(sorted(BRITISH, key=len, reverse=True)) + r")\b", re.I)


def respell(target, label):
    """米国綴りを英国綴りへ。`en` 以外はそのまま返す"""
    if target != "en":
        return label

    def one(m):
        w = m.group(0)
        b = BRITISH[w.lower()]
        return b[0].upper() + b[1:] if w[0].isupper() else b

    return _BRITISH_RE.sub(one, label)


def i18n_の訳(target: str) -> dict[str, str]:
    """**`ui/i18n/<言語>.json` に入っているリボンの語**(2026-08-21)。

    訳の置き場が2つあると必ずずれます。実際、2026-08-21 に2回踏みました
    — `OVERRIDES` だけ直して回すと、`gen_lang.py` が `ui/i18n` の古い語で
    上書きして戻します。

    *分担を決めました。*

    * `OVERRIDES["en"]` に載っている語(本家にどの言語でも無い、うちの
      ボタン)→ **訳は `ui/i18n/<言語>.json`**。i18n の手順で足します
    * `OVERRIDES["<言語>"]` に載っている語 → **その言語だけ本家に訳が
      無い**もの(ベトナム語は本家が 31% しか埋まっていないので 78 語)。
      ここでしか要らないので、ここに置きます

    どちらでも良い語はありません。重なっていた分は消しました。
    """
    if target == "en":
        return {}
    # **`ROOT` は vendor を指している。** ここは自分の隣の i18n を見る
    ここ = Path(__file__).resolve().parent
    p = ここ / "i18n" / f"{target}.json"
    kp = ここ / "i18n" / "keys.json"
    if not p.exists() or not kp.exists():
        return {}
    keys = json.loads(kp.read_text(encoding="utf-8"))
    番号 = {k["i"]: k["ja"] for k in keys}
    要る = set(OVERRIDES["en"])
    out = {}
    for x in json.loads(p.read_text(encoding="utf-8")):
        if not isinstance(x, dict) or not x.get("t"):
            continue
        ja = 番号.get(x["i"])
        if ja in 要る:
            out[ja] = x["t"]
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit("使い方: gen_ribbon_locale.py <locale>  (例: en)")
    target = sys.argv[1]
    # **その言語だけの穴埋め**を土台に、`ui/i18n` の訳を重ねます。
    # 重ねる順は gen_lang.py と同じ — 2つの道が同じ物を出すためです
    over = {**OVERRIDES.get(target, {}), **i18n_の訳(target)}
    doc_map = build_map(["documenteditor", "spreadsheeteditor"], target)
    cell_map = build_map(["spreadsheeteditor", "documenteditor"], target)
    tabs_of = tables_or_die(RIBBON)

    missing = []

    respelled = []

    def tr(label, m):
        if label in over:
            return over[label]
        if label not in m:
            missing.append(label)
            return label
        got = respell(target, m[label])
        if got != m[label]:
            respelled.append((m[label], got))
        return got

    out = []
    out.append(f"""//! リボンの {target} 版 — **語だけが ja(ribbon.rs)と違う**。
//! id・並び・ready・icon は ja と同一(ribbon.rs の試験が保証する)。
//!
//! このファイルは手で書かない:
//!
//! ```text
//! python3 ui/gen_ribbon_locale.py {target} > face/src/ribbon_{target}.rs
//! ```
//!
//! 対訳は vendor/web-apps のロケール(本家の語)。本家に無いこちらの
//! ボタンは gen_ribbon_locale.py の OVERRIDES 表で訳す。

use super::ribbon::{{{{取り込み}}}};
""")
    def q(s):
        """Rust のリテラルに戻す。解析器は逃げを解いた素の字を渡してくる"""
        return s.replace("\\", "\\\\").replace('"', '\\"')

    # **並びは WRITER → CALC。** 解析器の dict は CALC が先なので、そのまま
    # 回すと生成物の2つの表が入れ替わる(受け入れ試験で気づいた)
    for const in ("WRITER", "CALC"):
        m = doc_map if const == "WRITER" else cell_map
        out.append(f"pub const {const}: &[Tab] = &[")
        for tab in tabs_of[const]:
            out.append(f'    Tab {{ name: "{q(tr(tab.name, m))}", cmds: &[')
            for cmd in tab.cmds:
                # **書き方の名前をそのまま写す**(c / t / x / xt / xm)。
                # ボタンの性格は語ではないので、どの言語でも同じです
                if cmd.ready:
                    out.append(
                        f'        {cmd.kind}("{q(cmd.id)}", "{q(tr(cmd.label, m))}",'
                        f' "{q(cmd.icon)}"),')
                else:
                    out.append(
                        f'        {cmd.kind}("{q(tr(cmd.label, m))}", "{q(cmd.icon)}"),')
            out.append("    ]},")
        out.append("];\n")

    if missing:
        uniq = sorted(set(missing))
        sys.exit(
            f"訳の見つからない語が {len(uniq)} 個あります"
            f"(OVERRIDES に足してから出し直してください):\n  "
            + "\n  ".join(uniq))
    if target == "en" and not respelled:
        sys.exit(
            "::error::英語の綴り直しが1件も効いていません。"
            "本家の語が変わったか BRITISH 表が壊れています "
            "— 黙って米国綴りに戻さないため、ここで止めます")
    for a, b in sorted(set(respelled)):
        print(f"  綴り直し: {a} → {b}", file=sys.stderr)
    # **使った書き方だけを取り込む。** 使わない物を書くと警告になり、
    # clippy の門(-D warnings)で止まります
    本文 = "\n".join(out)
    使った = [k for k in ("c", "t", "x", "xt", "xm")
              if re.search(rf"^\s*{k}\(", 本文, re.M)]
    print(本文.replace("{取り込み}", ", ".join(使った + ["Tab"])))


if __name__ == "__main__":
    main()
