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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "顯示 0",
        "100%に拡大する": "縮放至 100%",
        "Python": "Python",
        "ふりがな": "注音",
        "やさしく": "更淺白",
        "インターフェイステーマ": "介面佈景主題",
        "ウォッチウィンドウ": "監看視窗",
        "オートSUM": "自動加總",
        "コメントを削除": "刪除註解",
        "ソルバー": "規劃求解",
        "テキストからデータ": "文字轉資料",
        "トレース矢印の削除": "移除箭號",
        "フィル": "填滿",
        "フィルターを解除": "清除篩選",
        "プラグインの管理": "管理外掛程式",
        "マクロを書く": "撰寫巨集",
        "ルビ": "注音標示",
        "区切り位置": "資料剖析",
        "印刷レイアウト": "整頁模式",
        "図表番号": "標號",
        "図表目次": "圖表目錄",
        "図表目次の更新": "更新圖表目錄",
        "均等割付": "分散對齊",
        "外部リンク(値で取り込む)": "外部連結(以值匯入)",
        "敬語にする": "更禮貌",
        "数学/三角": "數學與三角函數",
        "数式の表示": "顯示公式",
        "文字の向き(右横書き)": "文字方向(右起橫書)",
        "文字列操作": "文字操作",
        "日付/時刻": "日期與時間",
        "書き直す": "改寫",
        "最近使った関数": "最近使用的函數",
        "枠線も印刷": "列印格線",
        "目次の更新": "更新目錄",
        "続きを書く": "續寫",
        "縞模様の列": "帶狀欄",
        "翻訳": "翻譯",
        "表にする": "轉成表格",
        "要約": "摘要",
        "見出しも印刷": "列印標題",
        "計算方法": "計算方式",
        "詳細の非表示": "隱藏詳細資料",
        "重複の削除": "移除重複",
        "関数の挿入": "插入函數",
        "頼む": "提出需求",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "バージョン履歴": "版本歷程",
        "宛先": "目的地",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Mostra zeri",
        "100%に拡大する": "Zoom al 100%",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Più semplice",
        "インターフェイステーマ": "Tema dell'interfaccia",
        "ウォッチウィンドウ": "Finestra controllo celle",
        "オートSUM": "Somma automatica",
        "コメントを削除": "Elimina commento",
        "ソルバー": "Risolutore",
        "テキストからデータ": "Da testo a dati",
        "トレース矢印の削除": "Rimuovi frecce",
        "フィル": "Riempimento",
        "フィルターを解除": "Cancella filtro",
        "プラグインの管理": "Gestione plugin",
        "マクロを書く": "Scrivi macro",
        "ルビ": "Ruby",
        "区切り位置": "Testo in colonne",
        "印刷レイアウト": "Layout di stampa",
        "図表番号": "Didascalia",
        "図表目次": "Indice delle figure",
        "図表目次の更新": "Aggiorna indice delle figure",
        "均等割付": "Distribuito",
        "外部リンク(値で取り込む)": "Collegamenti esterni (importa come valori)",
        "敬語にする": "Più formale",
        "数学/三角": "Matematica e trigonometria",
        "数式の表示": "Mostra formule",
        "文字の向き(右横書き)": "Orientamento del testo (da destra a sinistra)",
        "文字列操作": "Testo",
        "日付/時刻": "Data e ora",
        "書き直す": "Riscrivi",
        "最近使った関数": "Usate di recente",
        "枠線も印刷": "Stampa griglia",
        "目次の更新": "Aggiorna sommario",
        "続きを書く": "Continua",
        "縞模様の列": "Colonne alternate",
        "翻訳": "Traduzione",
        "表にする": "In tabella",
        "要約": "Riassunto",
        "見出しも印刷": "Stampa intestazioni",
        "計算方法": "Calcolo",
        "詳細の非表示": "Nascondi dettaglio",
        "重複の削除": "Rimuovi duplicati",
        "関数の挿入": "Inserisci funzione",
        "頼む": "Chiedi",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "バージョン履歴": "Cronologia versioni",
        "共同編集モード": "Modalità collaborativa",
        "宛先": "Destinazione",
        "小計": "Subtotali",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Sıfırları göster",
        "100%に拡大する": "%100'e yakınlaştır",
        "Python": "Python",
        "ふりがな": "furigana",
        "やさしく": "Sadeleştir",
        "インターフェイステーマ": "Arayüz teması",
        "ウォッチウィンドウ": "Gözcü penceresi",
        "オートSUM": "Otomatik Toplam",
        "コメントを削除": "Açıklamayı sil",
        "ソルバー": "Çözücü",
        "テキストからデータ": "Metinden veri",
        "テキスト方向": "Metin yönü",
        "トレース矢印の削除": "Okları kaldır",
        "フィル": "Doldur",
        "フィルターを解除": "Filtreyi temizle",
        "プラグインの管理": "Eklentileri yönet",
        "マクロ": "Makrolar",
        "マクロを書く": "Makro yaz",
        "ルビ": "Ruby",
        "区切り位置": "Metni sütunlara dönüştür",
        "印刷レイアウト": "Yazdırma düzeni",
        "図表番号": "Resim yazısı",
        "図表目次": "Şekiller tablosu",
        "図表目次の更新": "Şekiller tablosunu güncelle",
        "均等割付": "Dağıtılmış",
        "外部リンク(値で取り込む)": "Dış bağlantılar (değer olarak al)",
        "敬語にする": "Daha kibar",
        "数学/三角": "Matematik ve Trigonometri",
        "数式の表示": "Formülleri göster",
        "文字の向き(右横書き)": "Metin yönü (sağdan sola)",
        "文字列操作": "Metin işlevleri",
        "日付/時刻": "Tarih ve Saat",
        "書き直す": "Yeniden yaz",
        "最近使った関数": "Son kullanılan işlevler",
        "枠線も印刷": "Kılavuz çizgileri de yazdır",
        "目次の更新": "İçindekiler tablosunu güncelle",
        "続きを書く": "Devamını yaz",
        "縞模様の列": "Şeritli sütunlar",
        "翻訳": "çeviri",
        "表にする": "Tabloya çevir",
        "要約": "özet",
        "見出しも印刷": "Başlıkları da yazdır",
        "計算方法": "Hesaplama",
        "詳細の非表示": "Ayrıntıyı gizle",
        "重複の削除": "Yinelenenleri kaldır",
        "関数の挿入": "İşlev ekle",
        "頼む": "Sor",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "バージョン履歴": "Sürüm geçmişi",
        "共同編集モード": "Birlikte düzenleme modu",
        "宛先": "Hedef",
        "小計": "Ara toplamlar",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Tampilkan nol",
        "100%に拡大する": "Zoom ke 100%",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Lebih sederhana",
        "インターフェイステーマ": "Tema antarmuka",
        "ウォッチウィンドウ": "Jendela pantauan",
        "オートSUM": "JumlahOtomatis",
        "コメントを削除": "Hapus komentar",
        "ソルバー": "Solver",
        "テキストからデータ": "Teks ke data",
        "テキスト方向": "Arah teks",
        "トレース矢印の削除": "Hapus panah",
        "フィル": "Isi",
        "フィルターを解除": "Bersihkan filter",
        "プラグインの管理": "Kelola plugin",
        "マクロ": "Makro",
        "マクロを書く": "Tulis makro",
        "ルビ": "Ruby",
        "区切り位置": "Teks ke kolom",
        "印刷レイアウト": "Tata letak cetak",
        "図表番号": "Keterangan",
        "図表目次": "Daftar gambar",
        "図表目次の更新": "Perbarui daftar gambar",
        "均等割付": "Terdistribusi",
        "外部リンク(値で取り込む)": "Tautan eksternal (impor sebagai nilai)",
        "敬語にする": "Lebih sopan",
        "数学/三角": "Matematika & Trigonometri",
        "数式の表示": "Tampilkan rumus",
        "文字の向き(右横書き)": "Arah teks (kanan-ke-kiri)",
        "文字列操作": "Teks",
        "日付/時刻": "Tanggal & Waktu",
        "書き直す": "Tulis ulang",
        "最近使った関数": "Baru digunakan",
        "枠線も印刷": "Cetak garis kisi",
        "目次の更新": "Perbarui daftar isi",
        "続きを書く": "Lanjutkan",
        "縞模様の列": "Kolom berpita",
        "翻訳": "Penerjemahan",
        "表にする": "Jadi tabel",
        "要約": "Peringkasan",
        "見出しも印刷": "Cetak judul",
        "計算方法": "Kalkulasi",
        "詳細の非表示": "Sembunyikan rincian",
        "重複の削除": "Hapus duplikat",
        "関数の挿入": "Sisipkan fungsi",
        "頼む": "Minta",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "チャット": "Obrolan",
        "デジタル署名を追加": "Tambahkan tanda tangan digital",
        "バージョン履歴": "Riwayat versi",
        "共同編集モード": "Mode pengeditan bersama",
        "宛先": "Tujuan",
        "小計": "Subtotal",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Hiển thị số 0",
        "100%に拡大する": "Thu phóng về 100%",
        "AI": "AI",
        "Python": "Python",
        "ふりがな": "furigana",
        "やさしく": "Dễ hiểu hơn",
        "インターフェイステーマ": "Chủ đề giao diện",
        "ウォッチウィンドウ": "Cửa sổ theo dõi",
        "オートSUM": "AutoSum",
        "コメントを削除": "Xóa nhận xét",
        "ソルバー": "Solver",
        "チェックボックス": "Hộp kiểm",
        "テキストからデータ": "Văn bản thành dữ liệu",
        "テキスト方向": "Hướng văn bản",
        "トレース矢印の削除": "Xóa mũi tên truy vết",
        "フィル": "Điền",
        "フィルターを解除": "Bỏ lọc",
        "プラグインの管理": "Quản lý plugin",
        "マクロ": "Macro",
        "マクロを書く": "Viết macro",
        "ルビ": "Ruby",
        "区切り位置": "Tách cột",
        "印刷レイアウト": "Bố cục in",
        "図表番号": "Chú thích hình/bảng",
        "図表目次": "Mục lục hình/bảng",
        "図表目次の更新": "Cập nhật mục lục hình/bảng",
        "均等割付": "Phân bố đều",
        "外部リンク(値で取り込む)": "Liên kết ngoài (nhập theo giá trị)",
        "小計": "Tổng phụ",
        "敬語にする": "Lịch sự hơn",
        "数学/三角": "Toán & Lượng giác",
        "数式の表示": "Hiển thị công thức",
        "文字の向き(右横書き)": "Hướng chữ (phải sang trái)",
        "文字列操作": "Xử lý văn bản",
        "日付/時刻": "Ngày & Giờ",
        "暗号化する": "Mã hóa",
        "書き直す": "Viết lại",
        "最近使った関数": "Dùng gần đây",
        "枠線も印刷": "In cả đường lưới",
        "目次の更新": "Cập nhật mục lục",
        "続きを書く": "Viết tiếp",
        "縞模様の列": "Cột sọc xen kẽ",
        "翻訳": "bản dịch",
        "表にする": "Thành bảng",
        "要約": "bản tóm tắt",
        "見出しも印刷": "In cả tiêu đề",
        "計算方法": "Cách tính",
        "詳細の非表示": "Ẩn chi tiết",
        "重複の削除": "Xóa trùng lặp",
        "関数の挿入": "Chèn hàm",
        "頼む": "Yêu cầu",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "チャット": "Trò chuyện",
        "共同編集モード": "Chế độ đồng soạn thảo",
        "宛先": "Đích",
        "操作を記録": "Ghi lại thao tác",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Nullen anzeigen",
        "100%に拡大する": "Auf 100 % zoomen",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Einfacher",
        "インターフェイステーマ": "Oberflächendesign",
        "ウォッチウィンドウ": "Überwachungsfenster",
        "オートSUM": "AutoSumme",
        "コメントを削除": "Kommentar löschen",
        "ソルバー": "Solver",
        "テキストからデータ": "Text zu Daten",
        "トレース矢印の削除": "Pfeile entfernen",
        "フィル": "Ausfüllen",
        "フィルターを解除": "Filter löschen",
        "プラグインの管理": "Plugins verwalten",
        "マクロを書く": "Makro schreiben",
        "ルビ": "Ruby",
        "区切り位置": "Text in Spalten",
        "印刷レイアウト": "Drucklayout",
        "図表番号": "Beschriftung",
        "図表目次": "Abbildungsverzeichnis",
        "図表目次の更新": "Abbildungsverzeichnis aktualisieren",
        "均等割付": "Verteilt",
        "外部リンク(値で取り込む)": "Externe Verknüpfungen (als Werte einlesen)",
        "敬語にする": "Höflicher",
        "数学/三角": "Math. u. Trigonom.",
        "数式の表示": "Formeln anzeigen",
        "文字の向き(右横書き)": "Textrichtung (rechts nach links)",
        "文字列操作": "Text",
        "日付/時刻": "Datum u. Uhrzeit",
        "書き直す": "Umschreiben",
        "最近使った関数": "Zuletzt verwendet",
        "枠線も印刷": "Gitternetzlinien drucken",
        "目次の更新": "Inhaltsverzeichnis aktualisieren",
        "続きを書く": "Fortsetzen",
        "縞模様の列": "Gebänderte Spalten",
        "翻訳": "Übersetzung",
        "表にする": "In Tabelle",
        "要約": "Zusammenfassung",
        "見出しも印刷": "Überschriften drucken",
        "計算方法": "Berechnung",
        "詳細の非表示": "Detail ausblenden",
        "重複の削除": "Duplikate entfernen",
        "関数の挿入": "Funktion einfügen",
        "頼む": "Anfragen",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "スタイル": "Typ",
        "バージョン履歴": "Versionsverlauf",
        "共同編集モード": "Co-Bearbeitung",
        "宛先": "Ziel",
        "小計": "Teilergebnisse",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Mostrar ceros",
        "100%に拡大する": "Zoom al 100%",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Más sencillo",
        "インターフェイステーマ": "Tema de la interfaz",
        "ウォッチウィンドウ": "Ventana Inspección",
        "オートSUM": "Autosuma",
        "コメントを削除": "Eliminar comentario",
        "ソルバー": "Solver",
        "テキストからデータ": "Texto a datos",
        "トレース矢印の削除": "Quitar flechas",
        "フィル": "Rellenar",
        "フィルターを解除": "Borrar filtro",
        "プラグインの管理": "Administrar complementos",
        "マクロを書く": "Escribir macro",
        "ルビ": "Ruby",
        "区切り位置": "Texto en columnas",
        "印刷レイアウト": "Diseño de impresión",
        "図表番号": "Título",
        "図表目次": "Tabla de ilustraciones",
        "図表目次の更新": "Actualizar tabla de ilustraciones",
        "均等割付": "Distribuido",
        "外部リンク(値で取り込む)": "Vínculos externos (importar como valores)",
        "敬語にする": "Más formal",
        "数学/三角": "Matemáticas y trigonometría",
        "数式の表示": "Mostrar fórmulas",
        "文字の向き(右横書き)": "Texto de derecha a izquierda",
        "文字列操作": "Texto",
        "日付/時刻": "Fecha y hora",
        "書き直す": "Reescribir",
        "最近使った関数": "Usadas recientemente",
        "枠線も印刷": "Imprimir líneas de cuadrícula",
        "目次の更新": "Actualizar tabla de contenido",
        "続きを書く": "Continuar",
        "縞模様の列": "Columnas con bandas",
        "翻訳": "Traducción",
        "表にする": "A tabla",
        "要約": "Resumen",
        "見出しも印刷": "Imprimir encabezados",
        "計算方法": "Cálculo",
        "詳細の非表示": "Ocultar detalle",
        "重複の削除": "Quitar duplicados",
        "関数の挿入": "Insertar función",
        "頼む": "Pedir",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "テキスト方向": "Dirección del texto",
        "デジタル署名を追加": "Agregar firma digital",
        "宛先": "Destino",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Afficher les zéros",
        "100%に拡大する": "Zoom à 100 %",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Plus simple",
        "インターフェイステーマ": "Thème de l'interface",
        "ウォッチウィンドウ": "Fenêtre Espion",
        "オートSUM": "Somme automatique",
        "コメントを削除": "Supprimer le commentaire",
        "ソルバー": "Solveur",
        "テキストからデータ": "Texte en données",
        "トレース矢印の削除": "Supprimer les flèches",
        "フィル": "Recopier",
        "フィルターを解除": "Effacer le filtre",
        "プラグインの管理": "Gérer les plugins",
        "マクロを書く": "Écrire une macro",
        "ルビ": "Ruby",
        "区切り位置": "Convertir",
        "印刷レイアウト": "Mise en page",
        "図表番号": "Légende",
        "図表目次": "Table des illustrations",
        "図表目次の更新": "Mettre à jour la table des illustrations",
        "均等割付": "Réparti",
        "外部リンク(値で取り込む)": "Liens externes (importer en valeurs)",
        "敬語にする": "Plus formel",
        "数学/三角": "Math et trigonométrie",
        "数式の表示": "Afficher les formules",
        "文字の向き(右横書き)": "Sens du texte (droite à gauche)",
        "文字列操作": "Texte",
        "日付/時刻": "Date et heure",
        "書き直す": "Réécrire",
        "最近使った関数": "Récemment utilisées",
        "枠線も印刷": "Imprimer le quadrillage",
        "目次の更新": "Mettre à jour la table des matières",
        "続きを書く": "Continuer",
        "縞模様の列": "Colonnes à bandes",
        "翻訳": "Traduction",
        "表にする": "En tableau",
        "要約": "Résumé",
        "見出しも印刷": "Imprimer les en-têtes",
        "計算方法": "Mode de calcul",
        "詳細の非表示": "Masquer le détail",
        "重複の削除": "Supprimer les doublons",
        "関数の挿入": "Insérer une fonction",
        "頼む": "Demander",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "チャット": "Discussion",
        "共同編集モード": "Mode coédition",
        "宛先": "Destination",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Mostrar zeros",
        "100%に拡大する": "Zoom de 100%",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Mais simples",
        "インターフェイステーマ": "Tema da interface",
        "ウォッチウィンドウ": "Janela de Inspeção",
        "オートSUM": "AutoSoma",
        "コメントを削除": "Excluir comentário",
        "ソルバー": "Solver",
        "テキストからデータ": "Texto para dados",
        "トレース矢印の削除": "Remover setas",
        "フィル": "Preencher",
        "フィルターを解除": "Limpar filtro",
        "プラグインの管理": "Gerenciar plugins",
        "マクロを書く": "Escrever macro",
        "ルビ": "Ruby",
        "区切り位置": "Texto para colunas",
        "印刷レイアウト": "Layout de impressão",
        "図表番号": "Legenda",
        "図表目次": "Índice de ilustrações",
        "図表目次の更新": "Atualizar índice de ilustrações",
        "均等割付": "Distribuído",
        "外部リンク(値で取り込む)": "Links externos (importar como valores)",
        "敬語にする": "Mais formal",
        "数学/三角": "Matemática e trigonometria",
        "数式の表示": "Mostrar fórmulas",
        "文字の向き(右横書き)": "Direção do texto (direita para a esquerda)",
        "文字列操作": "Texto",
        "日付/時刻": "Data e hora",
        "書き直す": "Reescrever",
        "最近使った関数": "Usadas recentemente",
        "枠線も印刷": "Imprimir linhas de grade",
        "目次の更新": "Atualizar sumário",
        "続きを書く": "Continuar o texto",
        "縞模様の列": "Colunas em tiras",
        "翻訳": "Tradução",
        "表にする": "Virar tabela",
        "要約": "Resumo",
        "見出しも印刷": "Imprimir títulos",
        "計算方法": "Cálculo",
        "詳細の非表示": "Ocultar detalhes",
        "重複の削除": "Remover duplicatas",
        "関数の挿入": "Inserir função",
        "頼む": "Pedir",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "デジタル署名を追加": "Adicionar assinatura digital",
        "バージョン履歴": "Histórico de versões",
        "宛先": "Destino",
        "折り返して全体を表示する": "Moldar texto",
        "範囲を足す": "Acrescentar intervalo",
        "読み取り専用を勧める": "Sugerir só de leitura",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Mostrar zeros",
        "100%に拡大する": "Ampliar para 100%",
        "Python": "Python",
        "ふりがな": "Furigana",
        "やさしく": "Mais simples",
        "インターフェイステーマ": "Tema da interface",
        "ウォッチウィンドウ": "Janela de inspeção",
        "オートSUM": "Soma automática",
        "コメントを削除": "Eliminar comentário",
        "ソルバー": "Solver",
        "テキストからデータ": "Dados a partir de texto",
        "テキスト方向": "Direção do texto",
        "トレース矢印の削除": "Remover setas",
        "フィル": "Preencher",
        "フィルターを解除": "Limpar filtro",
        "プラグインの管理": "Gerir plugins",
        "マクロ": "Macros",
        "マクロを書く": "Escrever macro",
        "ルビ": "Ruby",
        "区切り位置": "Texto para colunas",
        "印刷レイアウト": "Esquema de impressão",
        "図表番号": "Legenda",
        "図表目次": "Índice de ilustrações",
        "図表目次の更新": "Atualizar o índice de ilustrações",
        "均等割付": "Distribuído",
        "外部リンク(値で取り込む)": "Ligações externas (importar como valores)",
        "敬語にする": "Mais formal",
        "数学/三角": "Matemática e trigonometria",
        "数式の表示": "Mostrar fórmulas",
        "文字の向き(右横書き)": "Direção do texto (da direita para a esquerda)",
        "文字列操作": "Texto",
        "日付/時刻": "Data e hora",
        "書き直す": "Reescrever",
        "最近使った関数": "Funções recentes",
        "枠線も印刷": "Imprimir também as linhas de grelha",
        "目次の更新": "Atualizar o índice",
        "続きを書く": "Continuar a escrever",
        "縞模様の列": "Colunas às riscas",
        "翻訳": "Traduzir",
        "表にする": "Converter em tabela",
        "要約": "Resumir",
        "見出しも印刷": "Imprimir também os cabeçalhos",
        "計算方法": "Método de cálculo",
        "詳細の非表示": "Ocultar detalhes",
        "重複の削除": "Remover duplicados",
        "関数の挿入": "Inserir função",
        "頼む": "Pedir",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "チェックボックス": "Caixa de verificação",
        "チャット": "Conversa",
        "バージョン履歴": "Histórico de versões",
        "共同編集モード": "Modo de coedição",
        "宛先": "Destino",
        "折り返して全体を表示する": "Moldar o texto",
        "暗号化する": "Encriptar",
        "書式のコピー": "Copiar formato",
        "画面の文字を大きく": "Aumentar o texto do ecrã",
        "画面の文字を小さく": "Diminuir o texto do ecrã",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "Показывать нули",
        "100%に拡大する": "Масштаб 100%",
        "Python": "Python",
        "ふりがな": "Фуригана",
        "やさしく": "Проще",
        "インターフェイステーマ": "Тема интерфейса",
        "ウォッチウィンドウ": "Окно контрольного значения",
        "オートSUM": "Автосумма",
        "コメントを削除": "Удалить примечание",
        "ソルバー": "Поиск решения",
        "テキストからデータ": "Данные из текста",
        "トレース矢印の削除": "Убрать стрелки",
        "フィル": "Заполнить",
        "フィルターを解除": "Снять фильтр",
        "プラグインの管理": "Управление плагинами",
        "マクロを書く": "Написать макрос",
        "ルビ": "Руби",
        "区切り位置": "Текст по столбцам",
        "印刷レイアウト": "Разметка страницы",
        "図表番号": "Название",
        "図表目次": "Список иллюстраций",
        "図表目次の更新": "Обновить список иллюстраций",
        "均等割付": "Распределённый",
        "外部リンク(値で取り込む)": "Внешние ссылки (импорт значениями)",
        "敬語にする": "Вежливее",
        "数学/三角": "Математические",
        "数式の表示": "Показать формулы",
        "文字の向き(右横書き)": "Направление текста (справа налево)",
        "文字列操作": "Текстовые",
        "日付/時刻": "Дата и время",
        "書き直す": "Переписать",
        "最近使った関数": "Недавно использованные",
        "枠線も印刷": "Печатать сетку",
        "目次の更新": "Обновить оглавление",
        "続きを書く": "Продолжить",
        "縞模様の列": "Чередующиеся столбцы",
        "翻訳": "Перевод",
        "表にする": "В таблицу",
        "要約": "Сводка",
        "見出しも印刷": "Печатать заголовки",
        "計算方法": "Вычисления",
        "詳細の非表示": "Скрыть детали",
        "重複の削除": "Удалить дубликаты",
        "関数の挿入": "Вставка функции",
        "頼む": "Попросить",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "バージョン履歴": "Журнал версий",
        "共同編集モード": "Режим совместной работы",
        "宛先": "Адресат",
        "操作を記録": "Записывать действия",
        "暗号化する": "Зашифровать",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "0 표시",
        "100%に拡大する": "100%로 확대",
        "Python": "Python",
        "ふりがな": "후리가나",
        "やさしく": "쉽게",
        "インターフェイステーマ": "인터페이스 테마",
        "ウォッチウィンドウ": "조사식 창",
        "オートSUM": "자동 합계",
        "コメントを削除": "메모 삭제",
        "ソルバー": "해 찾기",
        "テキストからデータ": "텍스트에서 데이터로",
        "トレース矢印の削除": "연결선 제거",
        "フィル": "채우기",
        "フィルターを解除": "필터 해제",
        "プラグインの管理": "플러그인 관리",
        "マクロを書く": "매크로 작성",
        "ルビ": "루비",
        "区切り位置": "텍스트 나누기",
        "印刷レイアウト": "인쇄 모양",
        "図表番号": "캡션",
        "図表目次": "그림 목차",
        "図表目次の更新": "그림 목차 업데이트",
        "均等割付": "균등 분할",
        "外部リンク(値で取り込む)": "외부 링크(값으로 가져오기)",
        "敬語にする": "정중하게",
        "数学/三角": "수학/삼각",
        "数式の表示": "수식 표시",
        "文字の向き(右横書き)": "텍스트 방향(오른쪽부터 쓰기)",
        "文字列操作": "텍스트",
        "日付/時刻": "날짜/시간",
        "書き直す": "다시 쓰기",
        "最近使った関数": "최근에 사용한 함수",
        "枠線も印刷": "눈금선 인쇄",
        "目次の更新": "목차 업데이트",
        "続きを書く": "이어 쓰기",
        "縞模様の列": "줄무늬 열",
        "翻訳": "번역",
        "表にする": "표로 만들기",
        "要約": "요약",
        "見出しも印刷": "머리글 인쇄",
        "計算方法": "계산 방법",
        "詳細の非表示": "하위 수준 숨기기",
        "重複の削除": "중복된 항목 제거",
        "関数の挿入": "함수 삽입",
        "頼む": "요청",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "チェックボックス": "확인란",
        "デジタル署名を追加": "디지털 서명 추가",
        "宛先": "대상",
        "小計": "부분합",
        "操作を記録": "동작 기록",
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
            # **face へ移したときに落ちていた分**(2026-08-15 に戻した)。
        # 語は既存の ribbon_<loc>.rs から id と位置で拾い直した —
        # 生成器がこれを持っていないと、作り直した瞬間に訳が消える
        "0を表示する": "显示 0",
        "100%に拡大する": "缩放到 100%",
        "Python": "Python",
        "ふりがな": "注音",
        "やさしく": "更通俗",
        "インターフェイステーマ": "界面主题",
        "ウォッチウィンドウ": "监视窗口",
        "オートSUM": "自动求和",
        "コメントを削除": "删除批注",
        "ソルバー": "规划求解",
        "テキストからデータ": "文本转数据",
        "トレース矢印の削除": "删除追踪箭头",
        "フィル": "填充",
        "フィルターを解除": "清除筛选",
        "プラグインの管理": "管理插件",
        "マクロを書く": "编写宏",
        "ルビ": "注音",
        "区切り位置": "分列",
        "印刷レイアウト": "打印布局",
        "図表番号": "题注",
        "図表目次": "图表目录",
        "図表目次の更新": "更新图表目录",
        "均等割付": "分散对齐",
        "外部リンク(値で取り込む)": "外部链接(以值导入)",
        "敬語にする": "更礼貌",
        "数学/三角": "数学与三角函数",
        "数式の表示": "显示公式",
        "文字の向き(右横書き)": "文字方向(从右横排)",
        "文字列操作": "文本",
        "日付/時刻": "日期与时间",
        "書き直す": "改写",
        "最近使った関数": "最近使用的函数",
        "枠線も印刷": "打印网格线",
        "目次の更新": "更新目录",
        "続きを書く": "续写",
        "縞模様の列": "镶边列",
        "翻訳": "翻译",
        "表にする": "转为表格",
        "要約": "摘要",
        "見出しも印刷": "打印标题",
        "計算方法": "计算选项",
        "詳細の非表示": "隐藏明细",
        "重複の削除": "删除重复项",
        "関数の挿入": "插入函数",
        "頼む": "询问",
        # **手で良くした訳**(2026-08-15 に取り込んだ)。本家の語より
        # リボンに収まる・意味が近い、と人が直した分 — 表に無いと
        # 作り直すたびに本家の語へ戻ってしまう
        "テキスト方向": "文字方向",
        "宛先": "目标地址",
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
