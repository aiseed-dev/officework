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
RIBBON = Path(__file__).resolve().parent / "src/ribbon.rs"

# 本家に無い・こちらで足した語の対訳。ここに無い未解決語が出たら
# このスクリプトは止まる — その語をここに足してから出し直す
OVERRIDES = {
    "en": {
        "書式のコピー": "Format painter",
        "スタイル": "Style",
        "フィールドリスト": "Field list",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Bigger UI text",
        "画面の文字を小さく": "Smaller UI text",
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
        "操作を記録": "記錄操作",
        # 本家の台湾語は「尋找和引用」— **引用は大陸の言い方**。
        # こちらの台湾語の材料は 參照 26 回・引用 0 回で、台湾の Excel も
        # 「查閱與參照」(2026-08-11、分類の耳を訳した下請けが数えて指摘)
        "検索/行列": "查閱與參照",
        "書式のコピー": "複製格式",
        "スタイル": "樣式",
        "フィールドリスト": "欄位清單",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "放大介面文字",
        "画面の文字を小さく": "縮小介面文字",
        "ページ数": "頁數",
        "表のデザイン": "表格設計",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV 格式",
        "セルのロック": "鎖定儲存格",
        "データテーブル": "運算列表",
        "フラッシュフィル": "快速填入",
        "一覧": "清單",
        "名前を貼り付け": "貼上名稱",
        "復旧": "檔案復原",
        "折り返して全体を表示する": "自動換列",
        "控えの間隔": "備份間隔",
        "新しい .py": "新增 .py",
        "範囲を足す": "加入列印範圍",
        "紙に収める": "配合紙張大小",
        "紙の切れ目": "分頁線",
        "置き場を開く": "開啟資料夾",
        "計算の種類": "值的顯示方式",
        "許可する操作": "允許的操作",
        "読み取り専用を勧める": "建議唯讀",
    },
    "it": {
        "操作を記録": "Registra azioni",
        "書式のコピー": "Copia formato",
        "スタイル": "Stile",
        "フィールドリスト": "Elenco campi",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Ingrandisci testo dello schermo",
        "画面の文字を小さく": "Riduci testo dello schermo",
        "フィルタのボタン": "Pulsante filtro",
        "ヘッダー行": "Riga di intestazione",
        "合計行": "Riga totale",
        "最後の列": "Ultima colonna",
        "範囲に変換する": "Converti in intervallo",
        "表のデザイン": "Struttura tabella",
        "テーブルのサイズ変更": "Ridimensiona tabella",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Formato CSV",
        "セルのロック": "Blocca cella",
        "データテーブル": "Tabella dati",
        "フラッシュフィル": "Riempimento rapido",
        "一覧": "Elenco",
        "名前を貼り付け": "Incolla nome",
        "復旧": "Ripristino",
        "折り返して全体を表示する": "Testo a capo",
        "控えの間隔": "Intervallo copie",
        "新しい .py": "Nuovo .py",
        "範囲を足す": "Aggiungi intervallo",
        "紙に収める": "Adatta alla pagina",
        "紙の切れ目": "Fine pagina",
        "置き場を開く": "Apri cartella",
        "計算の種類": "Tipo di calcolo",
        "許可する操作": "Operazioni consentite",
        "読み取り専用を勧める": "Suggerisci sola lettura",
    },
    "tr": {
        "操作を記録": "İşlemleri kaydet",
        "書式のコピー": "Biçim boyacısı",
        "スタイル": "Stil",
        "フィールドリスト": "Alan listesi",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Ekran yazısını büyüt",
        "画面の文字を小さく": "Ekran yazısını küçült",
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
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV biçimi",
        "セルのロック": "Hücre kilidi",
        "データテーブル": "Veri tablosu",
        "フラッシュフィル": "Hızlı doldurma",
        "一覧": "Liste",
        "名前を貼り付け": "Ad yapıştır",
        "復旧": "Kurtarma",
        "折り返して全体を表示する": "Metni kaydır",
        "控えの間隔": "Yedek aralığı",
        "新しい .py": "Yeni .py",
        "範囲を足す": "Aralık ekle",
        "紙に収める": "Kâğıda sığdır",
        "紙の切れ目": "Sayfa sonları",
        "置き場を開く": "Klasörü aç",
        "計算の種類": "Hesaplama türü",
        "許可する操作": "İzin verilen işlemler",
        "読み取り専用を勧める": "Salt okunur öner",
    },
    "id": {
        "操作を記録": "Rekam tindakan",
        "書式のコピー": "Salin format",
        "スタイル": "Gaya",
        "フィールドリスト": "Daftar bidang",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Perbesar teks layar",
        "画面の文字を小さく": "Perkecil teks layar",
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
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Format CSV",
        "セルのロック": "Kunci sel",
        "データテーブル": "Tabel data",
        "フラッシュフィル": "Isi menurut contoh",
        "一覧": "Daftar",
        "名前を貼り付け": "Tempel nama",
        "復旧": "Pemulihan",
        "折り返して全体を表示する": "Bungkus teks",
        "控えの間隔": "Selang salinan",
        "新しい .py": ".py baru",
        "範囲を足す": "Tambah rentang",
        "紙に収める": "Muatkan ke kertas",
        "紙の切れ目": "Batas kertas",
        "置き場を開く": "Buka folder plugins",
        "計算の種類": "Jenis perhitungan",
        "許可する操作": "Tindakan yang diizinkan",
        "読み取り専用を勧める": "Sarankan hanya-baca",
    },
    "vi": {
        "操作を記録": "Ghi thao tác",
        "書式のコピー": "Sao chép định dạng",
        "スタイル": "Kiểu",
        "フィールドリスト": "Danh sách trường",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Phóng to chữ màn hình",
        "画面の文字を小さく": "Thu nhỏ chữ màn hình",
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
        "テキストの追加": "Thêm văn bản",
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
        "ページ数": "Số trang",
        "ページ番号": "Số trang hiện tại",
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
        "総計": "Tổng cộng",
        "縞模様の行": "Hàng xen kẽ màu",
        "罫線": "Viền",
        "蛍光ペン": "Bút dạ quang",
        "行番号を表示する": "Hiện số dòng",
        "表のデザイン": "Thiết kế bảng",
        "複合フィールド": "Trường phức hợp",
        "複数ページ": "Nhiều trang",
        "見出し": "Tiêu đề",
        "記号を挿入": "Chèn ký hiệu",
        "論理": "Lôgic",
        "財務": "Tài chính",
        "透かしを編集する": "Sửa hình mờ",
        "重複データを削除": "Xóa dữ liệu trùng lặp",
        "開く": "Mở",
        "電話番号": "Số điện thoại",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Dạng CSV",
        "セルのロック": "Khoá ô",
        "データテーブル": "Bảng dữ liệu",
        "フラッシュフィル": "Điền nhanh",
        "一覧": "Danh sách",
        "名前を貼り付け": "Dán tên",
        "復旧": "Khôi phục",
        "折り返して全体を表示する": "Ngắt dòng trong ô",
        "控えの間隔": "Tần suất sao lưu",
        "新しい .py": "Tệp .py mới",
        "範囲を足す": "Thêm vào vùng in",
        "紙に収める": "Thu vừa tờ giấy",
        "紙の切れ目": "Chỗ hết tờ giấy",
        "置き場を開く": "Mở thư mục",
        "計算の種類": "Cách hiện giá trị",
        "許可する操作": "Thao tác được phép",
        "読み取り専用を勧める": "Gợi ý chỉ đọc",
    },
    "de": {
        "操作を記録": "Aktionen aufzeichnen",
        "書式のコピー": "Format übertragen",
        "スタイル": "Stil",
        "フィールドリスト": "Feldliste",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Oberflächentext größer",
        "画面の文字を小さく": "Oberflächentext kleiner",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV-Format",
        "セルのロック": "Zelle sperren",
        "データテーブル": "Datentabelle",
        "フラッシュフィル": "Musterfüllung",
        "一覧": "Liste",
        "名前を貼り付け": "Namen einfügen",
        "復旧": "Wiederherstellen",
        "折り返して全体を表示する": "Text umbrechen",
        "控えの間隔": "Sicherungsintervall",
        "新しい .py": "Neue .py",
        "範囲を足す": "Bereich hinzufügen",
        "紙に収める": "Aufs Blatt einpassen",
        "紙の切れ目": "Blattgrenzen",
        "置き場を開く": "Ordner öffnen",
        "計算の種類": "Berechnungsart",
        "許可する操作": "Erlaubte Aktionen",
        "読み取り専用を勧める": "Schreibschutz empfehlen",
    },
    "es": {
        "操作を記録": "Grabar acciones",
        "書式のコピー": "Copiar formato",
        "スタイル": "Estilo",
        "フィールドリスト": "Lista de campos",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Agrandar texto de la pantalla",
        "画面の文字を小さく": "Reducir texto de la pantalla",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Formato CSV",
        "セルのロック": "Bloquear celda",
        "データテーブル": "Tabla de datos",
        "フラッシュフィル": "Relleno rápido",
        "一覧": "Lista",
        "名前を貼り付け": "Pegar nombre",
        "復旧": "Recuperar",
        "折り返して全体を表示する": "Ajustar texto",
        "控えの間隔": "Intervalo de copias",
        "新しい .py": "Nuevo .py",
        "範囲を足す": "Añadir rango",
        "紙に収める": "Ajustar al papel",
        "紙の切れ目": "Cortes del papel",
        "置き場を開く": "Abrir carpeta",
        "計算の種類": "Tipo de cálculo",
        "許可する操作": "Operaciones permitidas",
        "読み取り専用を勧める": "Sugerir solo lectura",
    },
    "fr": {
        "操作を記録": "Enregistrer les actions",
        "書式のコピー": "Copier le format",
        "スタイル": "Style",
        "フィールドリスト": "Liste des champs",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Agrandir le texte de l'écran",
        "画面の文字を小さく": "Réduire le texte de l'écran",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Format CSV",
        "セルのロック": "Verrouiller la cellule",
        "データテーブル": "Table de données",
        "フラッシュフィル": "Remplissage instantané",
        "一覧": "Liste",
        "名前を貼り付け": "Coller un nom",
        "復旧": "Récupérer",
        "折り返して全体を表示する": "Renvoyer à la ligne",
        "控えの間隔": "Intervalle des copies",
        "新しい .py": "Nouveau .py",
        "範囲を足す": "Ajouter la plage",
        "紙に収める": "Ajuster au papier",
        "紙の切れ目": "Sauts de page",
        "置き場を開く": "Ouvrir le dossier",
        "計算の種類": "Type de calcul",
        "許可する操作": "Actions permises",
        "読み取り専用を勧める": "Suggérer la lecture seule",
    },
    "pt-br": {
        "操作を記録": "Gravar ações",
        # ブラジル**だけ**を分ける札(2026-08-11 発注者)
        # 本家のブラジル語そのものが誤っていた3語。ブラジル語としても
        # 誤りなので、欧州版と一緒に直す(2026-08-11):
        #   Projeto da mesa   = 家具の机の設計(table を家具と取った)
        #   Total de linhas   = 行数(「合計の行」ではない)
        #   Faixa de proteção = 保護の帯(命令の動詞が要る所を名詞句に)
        "表のデザイン": "Design da Tabela",
        "合計行": "Linha de Totais",
        "範囲を保護する": "Proteger Intervalo",
        "書式のコピー": "Copiar formato",
        "スタイル": "Estilo",
        "フィールドリスト": "Lista de campos",
        "画面の文字を大きく": "Aumentar texto da tela",
        "画面の文字を小さく": "Diminuir texto da tela",
        "CSV の形": "Formato CSV",
        "セルのロック": "Bloquear célula",
        "データテーブル": "Tabela de dados",
        "フラッシュフィル": "Preencher pelo exemplo",
        "一覧": "Lista",
        "名前を貼り付け": "Colar nome",
        "復旧": "Recuperar",
        "折り返して全体を表示する": "Quebrar texto",
        "控えの間隔": "Intervalo das cópias",
        "新しい .py": "Novo .py",
        "範囲を足す": "Adicionar intervalo",
        "紙に収める": "Ajustar ao papel",
        "紙の切れ目": "Quebras de página",
        "置き場を開く": "Abrir a pasta",
        "計算の種類": "Mostrar os valores como",
        "許可する操作": "Operações permitidas",
        "読み取り専用を勧める": "Sugerir somente leitura",
    },
    "pt": {
        "操作を記録": "Gravar ações",
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
        # ポルトガル。**素の pt は欧州** — アンゴラ・モザンビーク等も
        # こちらに落ちる(分岐しているのはブラジルだけ)
        "書式のコピー": "Copiar formatação",
        "スタイル": "Estilo",
        "フィールドリスト": "Lista de campos",
        "画面の文字を大きく": "Aumentar texto do ecrã",
        "画面の文字を小さく": "Diminuir texto do ecrã",
        "CSV の形": "Formato CSV",
        "セルのロック": "Bloquear célula",
        "データテーブル": "Tabela de dados",
        "フラッシュフィル": "Preencher pelo exemplo",
        "一覧": "Lista",
        "名前を貼り付け": "Colar nome",
        "復旧": "Recuperar",
        "折り返して全体を表示する": "Moldar texto",
        "控えの間隔": "Intervalo das cópias",
        "新しい .py": "Novo .py",
        "範囲を足す": "Acrescentar intervalo",
        "紙に収める": "Ajustar ao papel",
        "紙の切れ目": "Quebras de página",
        "置き場を開く": "Abrir a pasta",
        "計算の種類": "Mostrar os valores como",
        "許可する操作": "Operações permitidas",
        "読み取り専用を勧める": "Sugerir só de leitura",
    },
    "ru": {
        "操作を記録": "Записать действия",
        "書式のコピー": "Формат по образцу",
        "スタイル": "Стиль",
        "フィールドリスト": "Список полей",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "Крупнее текст интерфейса",
        "画面の文字を小さく": "Мельче текст интерфейса",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "Формат CSV",
        "セルのロック": "Блокировка ячейки",
        "データテーブル": "Таблица данных",
        "フラッシュフィル": "Мгновенное заполнение",
        "一覧": "Список",
        "名前を貼り付け": "Вставить имя",
        "復旧": "Восстановление",
        "折り返して全体を表示する": "Перенос текста",
        "控えの間隔": "Интервал копий",
        "新しい .py": "Новый .py",
        "範囲を足す": "Добавить в область печати",
        "紙に収める": "Уместить на листе",
        "紙の切れ目": "Разрывы страниц",
        "置き場を開く": "Открыть папку",
        "計算の種類": "Как считать",
        "許可する操作": "Разрешённые действия",
        "読み取り専用を勧める": "Рекомендовать только чтение",
    },
    "ko": {
        "操作を記録": "조작 기록",
        "書式のコピー": "서식 복사",
        "スタイル": "스타일",
        "フィールドリスト": "필드 목록",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "화면 글자 크게",
        "画面の文字を小さく": "화면 글자 작게",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV 형식",
        "セルのロック": "셀 잠금",
        "データテーブル": "데이터 표",
        "フラッシュフィル": "빠른 채우기",
        "一覧": "목록",
        "名前を貼り付け": "이름 붙여넣기",
        "復旧": "복구",
        "折り返して全体を表示する": "텍스트 줄 바꿈",
        "控えの間隔": "사본 간격",
        "新しい .py": "새 .py",
        "範囲を足す": "범위 더하기",
        "紙に収める": "종이에 맞추기",
        "紙の切れ目": "종이 끊기는 자리",
        "置き場を開く": "폴더 열기",
        "計算の種類": "계산 종류",
        "許可する操作": "허용할 동작",
        "読み取り専用を勧める": "읽기 전용 권하기",
    },
    "zh": {
        "操作を記録": "记录操作",
        "書式のコピー": "格式刷",
        "スタイル": "样式",
        "フィールドリスト": "字段列表",
        # 表示タブ(こちらで足したボタン — 画面の文字の大きさ)
        "画面の文字を大きく": "放大界面文字",
        "画面の文字を小さく": "缩小界面文字",
        # 2026-08-10 に足した21語(台帳の消し込みで増えたボタン)
        "CSV の形": "CSV 格式",
        "セルのロック": "锁定单元格",
        "データテーブル": "数据表",
        "フラッシュフィル": "快速填充",
        "一覧": "列表",
        "名前を貼り付け": "粘贴名称",
        "復旧": "恢复",
        "折り返して全体を表示する": "自动换行",
        "控えの間隔": "备份间隔",
        "新しい .py": "新建 .py",
        "範囲を足す": "添加范围",
        "紙に収める": "适合纸张",
        "紙の切れ目": "分页处",
        "置き場を開く": "打开文件夹",
        "計算の種類": "值的显示方式",
        "許可する操作": "允许的操作",
        "読み取り専用を勧める": "建议只读",
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


def main():
    if len(sys.argv) != 2:
        sys.exit("使い方: gen_ribbon_locale.py <locale>  (例: en)")
    target = sys.argv[1]
    over = OVERRIDES.get(target, {})
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

use super::ribbon::{{c, x, Tab}};
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
                if cmd.ready:
                    out.append(
                        f'        c("{q(cmd.id)}", "{q(tr(cmd.label, m))}", "{q(cmd.icon)}"),')
                else:
                    out.append(f'        x("{q(tr(cmd.label, m))}", "{q(cmd.icon)}"),')
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
    print("\n".join(out))


if __name__ == "__main__":
    main()
