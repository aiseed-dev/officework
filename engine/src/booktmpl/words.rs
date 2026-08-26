//! **テンプレートの言葉の表(15言語)。**
//!
//! *この表は `python3 ui/gen_tmpl_words.py` が起こします。手で書かないでください。*
//!
//! 訳の出どころは `ui/i18n/<loc>.json` の1つです。`kumihan` は `lang` に
//! 依存しないので(組版のエンジンが言語の表を引きずらないため)、
//! ここへ写して持ちます。
//!
//! **書くときは画面の言語、読むときはどの言語でも受けます。** 配られた
//! テンプレートを別の国の人が開いても読めないと困るからです。

use crate::font::default_language;

/// 表が持っている言語の札(並びは下の表の桁と同じ)
pub const LANGS: &[&str] = &["de", "en", "es", "fr", "id", "it", "ja", "ko", "pt", "pt-br", "ru", "tr", "vi", "zh", "zh-tw"];

/// (記号, 言語ごとの訳)。桁の並びは [`LANGS`] と同じです
pub const WORDS: &[(&str, [&str; 15])] = &[
    ("paper", ["Papier", "Paper", "Papel", "Papier", "Kertas", "Carta", "用紙", "용지", "Papel", "Papel", "Бумага", "Kâğıt", "Giấy", "纸张", "紙張"]),
    ("col_width", ["Column width", "Column width", "Column width", "Column width", "Column width", "Column width", "列幅", "Column width", "Column width", "Column width", "Column width", "Column width", "Column width", "Column width", "Column width"]),
    ("row_height", ["Row height", "Row height", "Row height", "Row height", "Row height", "Row height", "行の高さ", "Row height", "Row height", "Row height", "Row height", "Row height", "Row height", "Row height", "Row height"]),
    ("print", ["Drucken", "Print", "Imprimir", "Imprimer", "Cetak", "Stampa", "印刷", "인쇄", "Imprimir", "Imprimir", "Печать", "Yazdır", "In", "打印", "列印"]),
    ("page_break", ["Page break", "Page break", "Page break", "Page break", "Page break", "Page break", "改ページ", "Page break", "Page break", "Page break", "Page break", "Page break", "Page break", "Page break", "Page break"]),
    ("header_footer", ["Kopf- und Fußzeile", "Header and footer", "Encabezado y pie", "En-tête et pied de page", "Header dan footer", "Intestazione e piè di pagina", "ヘッダーとフッター", "머리글과 바닥글", "Cabeçalho e rodapé", "Cabeçalho e rodapé", "Колонтитулы", "Üst bilgi ve alt bilgi", "Đầu trang và chân trang", "页眉和页脚", "頁首及頁尾"]),
    ("view", ["View", "View", "View", "View", "View", "View", "画面", "View", "View", "View", "View", "View", "View", "View", "View"]),
    ("tmpl_group", ["Group", "Group", "Group", "Group", "Group", "Group", "グループ化", "Group", "Group", "Group", "Group", "Group", "Group", "Group", "Group"]),
    ("tmpl_protect", ["Protection", "Protection", "Protection", "Protection", "Protection", "Protection", "保護", "Protection", "Protection", "Protection", "Protection", "Protection", "Protection", "Protection", "Protection"]),
    ("format", ["Format", "Format", "Format", "Format", "Format", "Format", "書式", "Format", "Format", "Format", "Format", "Format", "Format", "Format", "Format"]),
    ("format_applied", ["Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "書式の当て", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to", "Format applied to"]),
    ("workbook", ["Workbook", "Workbook", "Workbook", "Workbook", "Workbook", "Workbook", "ブック", "Workbook", "Workbook", "Workbook", "Workbook", "Workbook", "Workbook", "Workbook", "Workbook"]),
    ("sheets", ["Blätter", "Sheets", "Hojas", "Feuilles", "Lembar kerja", "Fogli", "シート", "시트", "Folhas", "Planilhas", "Листы", "Çalışma sayfaları", "Trang tính", "工作表", "工作表"]),
    ("tmpl_column", ["Column", "Column", "Column", "Column", "Column", "Column", "列", "Column", "Column", "Column", "Column", "Column", "Column", "Column", "Column"]),
    ("width_2", ["Breite", "width", "Ancho", "la largeur", "lebar", "tutta la larghezza", "幅", "너비", "largura", "largura", "Ширина", "Genişlik", "chiều rộng", "宽度", "寬度"]),
    ("row", ["Row", "Row", "Row", "Row", "Row", "Row", "行", "Row", "Row", "Row", "Row", "Row", "Row", "Row", "Row"]),
    ("height", ["Höhe", "Height", "Alto", "Hauteur", "Tinggi", "Altezza", "高さ", "높이", "Altura", "Altura", "Высота", "Yükseklik", "Chiều cao", "高度", "高度"]),
    ("size", ["Size", "Size", "Size", "Size", "Size", "Size", "大きさ", "Size", "Size", "Size", "Size", "Size", "Size", "Size", "Size"]),
    ("orientation", ["Ausrichtung", "Orientation", "Orientación", "Orientation", "Orientasi", "Orientamento", "向き", "방향", "Orientação", "Orientação", "Ориентация", "Yön", "Hướng", "方向", "方向"]),
    ("margins", ["Ränder", "Margins", "Márgenes", "Marges", "Margin", "Margini", "余白", "여백", "Margens", "Margens", "Поля", "Kenar boşluğu", "Lề", "页边距", "邊界"]),
    ("gridlines", ["Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "目盛線", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines", "Gridlines"]),
    ("tmpl_zoom", ["Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "拡大", "Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "Zoom"]),
    ("scale", ["Scale", "Scale", "Scale", "Scale", "Scale", "Scale", "倍率", "Scale", "Scale", "Scale", "Scale", "Scale", "Scale", "Scale", "Scale"]),
    ("fit_to_width", ["Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "横に収める", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width", "Fit to width"]),
    ("fit_to_height", ["Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "縦に収める", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height"]),
    ("row_col_headings", ["Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "行列番号", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings", "Row and column headings"]),
    ("title_rows", ["Title rows", "Title rows", "Title rows", "Title rows", "Title rows", "Title rows", "タイトル行", "Title rows", "Title rows", "Title rows", "Title rows", "Title rows", "Title rows", "Title rows", "Title rows"]),
    ("title_cols", ["Title columns", "Title columns", "Title columns", "Title columns", "Title columns", "Title columns", "タイトル列", "Title columns", "Title columns", "Title columns", "Title columns", "Title columns", "Title columns", "Title columns", "Title columns"]),
    ("position", ["Position", "Position", "Posición", "Position", "Posisi", "Posizione", "位置", "위치", "Posição", "Posição", "Положение", "Konum", "Vị trí", "位置", "位置"]),
    ("tmpl_text", ["Text", "Text", "Text", "Text", "Text", "Text", "文字", "Text", "Text", "Text", "Text", "Text", "Text", "Text", "Text"]),
    ("freeze", ["Freeze", "Freeze", "Freeze", "Freeze", "Freeze", "Freeze", "固定", "Freeze", "Freeze", "Freeze", "Freeze", "Freeze", "Freeze", "Freeze", "Freeze"]),
    ("formula_2", ["Formel", "Formula", "Ecuación", "Équation", "Rumus", "Formula", "数式", "수식", "Equação", "Equação", "Формула", "Denklem", "Công thức", "公式", "公式"]),
    ("rtl", ["Right to left", "Right to left", "Right to left", "Right to left", "Right to left", "Right to left", "右横書き", "Right to left", "Right to left", "Right to left", "Right to left", "Right to left", "Right to left", "Right to left", "Right to left"]),
    ("hide", ["Ausblenden", "Hide", "Ocultar", "Masquer", "Sembunyikan", "Nascondi", "非表示", "숨기기", "Ocultar", "Ocultar", "Скрыть", "Gizle", "Ẩn", "隐藏", "隱藏"]),
    ("tab_color", ["Tab color", "Tab color", "Tab color", "Tab color", "Tab color", "Tab color", "見出しの色", "Tab color", "Tab color", "Tab color", "Tab color", "Tab color", "Tab color", "Tab color", "Tab color"]),
    ("default_col_width", ["Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "既定の列幅", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width"]),
    ("default_row_height", ["Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "既定の行の高さ", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height"]),
    ("kind", ["Kind", "Kind", "Kind", "Kind", "Kind", "Kind", "種類", "Kind", "Kind", "Kind", "Kind", "Kind", "Kind", "Kind", "Kind"]),
    ("level", ["Level", "Level", "Level", "Level", "Level", "Level", "段", "Level", "Level", "Level", "Level", "Level", "Level", "Level", "Level"]),
    ("tmpl_collapsed", ["Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "畳む", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed", "Collapsed"]),
    ("allowed_actions", ["Erlaubte Aktionen", "Allowed actions", "Operaciones permitidas", "Actions permises", "Tindakan yang diizinkan", "Operazioni consentite", "許可する操作", "허용할 동작", "Operações permitidas", "Operações permitidas", "Разрешённые действия", "İzin verilen işlemler", "Thao tác được phép", "允许的操作", "允許的操作"]),
    ("name", ["Umbenennen", "Name", "Renombrar", "Renommer", "Ganti nama", "Rinomina", "名前", "이름 변경", "Mudar o nome", "Renomear", "Переименовать", "Yeniden adlandır", "Đổi tên", "重命名", "重新命名"]),
    ("item", ["Item", "Item", "Item", "Item", "Item", "Item", "項目", "Item", "Item", "Item", "Item", "Item", "Item", "Item", "Item"]),
    ("value", ["Wert", "Value", "Valor", "Valeur", "Nilai", "Valore", "値", "값", "Valor", "Valor", "Значение", "Değer", "Giá trị", "值", "值"]),
    ("range", ["Range", "Range", "Range", "Range", "Range", "Range", "範囲", "Range", "Range", "Range", "Range", "Range", "Range", "Range", "Range"]),
    ("landscape_2", ["Querformat", "Landscape", "Horizontal", "paysage", "Lanskap", "Orizzontale", "横", "가로", "Horizontal", "Paisagem", "Альбомная", "Yatay", "Ngang", "横向", "橫向"]),
    ("portrait", ["Hochformat", "Portrait", "Vertical", "portrait", "Potret", "Verticale", "縦", "세로", "Vertical", "Retrato", "Книжная", "Dikey", "Dọc", "纵向", "直向"]),
    ("header", ["Kopfzeile", "header", "encabezado", "en-tête", "kepala halaman", "intestazione", "ヘッダー", "머리글", "cabeçalho", "cabeçalho", "Верхний колонтитул", "Üstbilgi", "đầu trang", "页眉", "頁首"]),
    ("footer", ["Fußzeile", "footer", "pie de página", "pied de page", "kaki halaman", "piè di pagina", "フッター", "바닥글", "rodapé", "rodapé", "Нижний колонтитул", "Altbilgi", "chân trang", "页脚", "頁尾"]),
    ("header_even", ["Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "偶数ヘッダー", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header"]),
    ("footer_even", ["Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "偶数フッター", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer"]),
    ("header_first", ["First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "先頭ヘッダー", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header"]),
    ("footer_first", ["First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "先頭フッター", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer"]),
    ("theme_colors", ["Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "テーマ色", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors", "Theme colors"]),
    ("show_r1c1", ["Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "R1C1 で見せる", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1", "Show as R1C1"]),
    ("select_locked_cells", ["Gesperrte Zellen auswählen", "Select locked cells", "Seleccionar celdas bloqueadas", "Sélectionner les cellules verrouillées", "Pilih sel terkunci", "Selezione celle bloccate", "ロックされたセルの選択", "잠긴 셀 선택", "Selecionar células bloqueadas", "Selecionar células bloqueadas", "Выделение заблокированных ячеек", "Kilitli hücreleri seçme", "Chọn ô đã khoá", "选定锁定的单元格", "選取鎖定的儲存格"]),
    ("select_unlocked_cells", ["Nicht gesperrte Zellen auswählen", "Select unlocked cells", "Seleccionar celdas desbloqueadas", "Sélectionner les cellules déverrouillées", "Pilih sel tidak terkunci", "Selezione celle sbloccate", "ロックされていないセルの選択", "잠기지 않은 셀 선택", "Selecionar células desbloqueadas", "Selecionar células desbloqueadas", "Выделение незаблокированных ячеек", "Kilitli olmayan hücreleri seçme", "Chọn ô chưa khoá", "选定未锁定的单元格", "選取未鎖定的儲存格"]),
    ("format_cells", ["Zellen formatieren", "Format cells", "Dar formato a celdas", "Format des cellules", "Format sel", "Formatta celle", "セルの書式設定", "셀 서식", "Formatar células", "Formatar celulas", "Форматировать ячейки", "Hücreleri biçimlendir", "Định dạng ô", "单元格格式", "格式化儲存格"]),
    ("format_columns", ["Spalten formatieren", "Format columns", "Aplicar formato a columnas", "Format de colonne", "Format kolom", "Formattazione colonne", "列の書式設定", "열 서식", "Formatar colunas", "Formatar colunas", "Формат столбцов", "Sütunları biçimlendirme", "Định dạng cột", "设置列格式", "欄格式"]),
    ("format_rows", ["Zeilen formatieren", "Format rows", "Aplicar formato a filas", "Format de ligne", "Format baris", "Formattazione righe", "行の書式設定", "행 서식", "Formatar linhas", "Formatar linhas", "Формат строк", "Satırları biçimlendirme", "Định dạng hàng", "设置行格式", "列格式"]),
    ("insert_columns", ["Spalten einfügen", "Insert columns", "Insertar columnas", "Insérer des colonnes", "Sisipkan kolom", "Inserimento colonne", "列の挿入", "열 삽입", "Inserir colunas", "Inserir colunas", "Вставка столбцов", "Sütun ekleme", "Chèn cột", "插入列", "插入欄"]),
    ("insert_rows", ["Zeilen einfügen", "Insert rows", "Insertar filas", "Insérer des lignes", "Sisipkan baris", "Inserimento righe", "行の挿入", "행 삽입", "Inserir linhas", "Inserir linhas", "Вставка строк", "Satır ekleme", "Chèn hàng", "插入行", "插入列"]),
    ("insert_hyperlinks", ["Hyperlinks einfügen", "Insert hyperlinks", "Insertar hipervínculos", "Insérer des liens hypertexte", "Sisipkan hyperlink", "Inserimento collegamenti ipertestuali", "ハイパーリンクの挿入", "하이퍼링크 삽입", "Inserir hiperligações", "Inserir hiperlinks", "Вставка гиперссылок", "Köprü ekleme", "Chèn siêu liên kết", "插入超链接", "插入超連結"]),
    ("delete_columns", ["Spalten löschen", "Delete columns", "Eliminar columnas", "Supprimer des colonnes", "Hapus kolom", "Eliminazione colonne", "列の削除", "열 삭제", "Eliminar colunas", "Excluir colunas", "Удаление столбцов", "Sütun silme", "Xóa cột", "删除列", "刪除欄"]),
    ("delete_rows", ["Zeilen löschen", "Delete rows", "Eliminar filas", "Supprimer des lignes", "Hapus baris", "Eliminazione righe", "行の削除", "행 삭제", "Eliminar linhas", "Excluir linhas", "Удаление строк", "Satır silme", "Xóa hàng", "删除行", "刪除列"]),
    ("sort_2", ["Sortieren", "Sort", "Ordenar", "Trier", "Urutkan", "Ordinamento", "並べ替え", "정렬", "Ordenar", "Classificar", "Сортировка", "Sıralama", "Sắp xếp", "排序", "排序"]),
    ("use_autofilter", ["AutoFilter verwenden", "Use AutoFilter", "Usar Autofiltro", "Utiliser le filtre automatique", "Gunakan Filter Otomatis", "Uso del filtro automatico", "オートフィルターの使用", "자동 필터 사용", "Usar Filtro Automático", "Usar AutoFiltro", "Использование автофильтра", "Otomatik Filtre kullanma", "Dùng bộ lọc tự động", "使用自动筛选", "使用自動篩選"]),
    ("use_pivottable", ["Pivot-Tabellen verwenden", "Use PivotTable", "Usar tablas dinámicas", "Utiliser les tableaux croisés dynamiques", "Gunakan Pivot Table", "Uso della tabella pivot", "ピボットテーブルの使用", "피벗 테이블 사용", "Usar Tabela Dinâmica", "Usar Tabela Dinâmica", "Использование сводных таблиц", "PivotTable kullanma", "Dùng pivot table", "使用数据透视表", "使用樞紐分析表"]),
    ("edit_objects", ["Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "オブジェクトの編集", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects", "Edit objects"]),
    ("bold", ["Fett", "Bold", "Negrita", "Gras", "Tebal", "Grassetto", "太字", "굵게", "Negrito", "Negrito", "Полужирный", "Kalın", "Đậm", "加粗", "粗體"]),
    ("italic", ["Kursiv", "Italic", "Cursiva", "Italique", "Miring", "Corsivo", "斜体", "기울임꼴", "Itálico", "Itálico", "Курсив", "İtalik", "Nghiêng", "倾斜", "斜體"]),
    ("underline", ["Unterstrichen", "Underline", "Subrayado", "Souligné", "Garis bawah", "Sottolineato", "下線", "밑줄", "Sublinhado", "Sublinhado", "Подчёркнутый", "Altı çizili", "Gạch dưới", "下划线", "底線"]),
    ("strikethrough", ["Durchgestrichen", "Strikethrough", "Tachado", "Barré", "Coret", "Barrato", "取り消し線", "취소선", "Rasurado", "Tachado", "Зачёркнутый", "Üstü çizili", "Gạch ngang", "删除线", "刪除線"]),
    ("subscript", ["Tiefgestellt", "Subscript", "Subíndice", "Indice", "Subskrip", "Pedice", "下付き", "아래 첨자", "Inferior à linha", "Subscrito", "Подстрочный", "Alt simge", "Chỉ số dưới", "下标", "下標"]),
    ("tmpl_borders", ["Borders", "Borders", "Borders", "Borders", "Borders", "Borders", "罫線", "Borders", "Borders", "Borders", "Borders", "Borders", "Borders", "Borders", "Borders"]),
    ("halign", ["Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "横位置", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment", "Horizontal alignment"]),
    ("valign", ["Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "縦位置", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment", "Vertical alignment"]),
    ("fill_color", ["Füllung", "Fill colour", "Relleno", "Remplissage", "Isian", "Riempimento", "塗り", "채우기", "Preenchimento", "Preenchimento", "Заливка", "Dolgu", "Màu tô", "填充", "填滿"]),
    ("fill_bg", ["Fill background", "Fill background", "Fill background", "Fill background", "Fill background", "Fill background", "塗りの地", "Fill background", "Fill background", "Fill background", "Fill background", "Fill background", "Fill background", "Fill background", "Fill background"]),
    ("fill_pattern", ["Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "塗りの柄", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern", "Fill pattern"]),
    ("gradient_2", ["Verlauf", "Gradient", "Degradado", "Dégradé", "Gradasi", "Sfumatura", "グラデーション", "그라데이션", "Gradiente", "Gradiente", "Градиент", "Geçiş", "Chuyển sắc", "渐变", "漸層"]),
    ("fill_theme", ["Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "塗りのテーマ色", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color"]),
    ("font_color", ["Font color", "Font color", "Font color", "Font color", "Font color", "Font color", "文字色", "Font color", "Font color", "Font color", "Font color", "Font color", "Font color", "Font color", "Font color"]),
    ("color_theme", ["Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "文字のテーマ色", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color"]),
    ("tmpl_font", ["Font", "Font", "Font", "Font", "Font", "Font", "書体", "Font", "Font", "Font", "Font", "Font", "Font", "Font", "Font"]),
    ("rotation_2", ["Drehung", "Rotation", "Giro", "Rotation", "Putaran", "Rotazione", "回転", "회전", "Rotação", "Rotação", "Поворот", "Döndürme", "Xoay", "旋转", "旋轉"]),
    ("wrap", ["Zeilenumbruch", "Wrap", "Ajustar", "Renvoi à la ligne", "Bungkus", "Testo a capo", "折り返して全体を表示", "줄 바꿈", "Moldar texto", "Quebrar texto", "Перенос текста", "Kaydır", "Ngắt dòng", "自动换行", "自動換列"]),
    ("shrink", ["Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "縮小", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit", "Shrink to fit"]),
    ("indent_3", ["Einzug", "Indent", "Sangría", "Retrait", "Indentasi", "Rientro", "字下げ", "들여쓰기", "Avanço", "Recuo", "Отступ", "Girinti", "Thụt lề", "缩进", "縮排"]),
    ("number_format", ["Zahlenformat", "Number format", "Formato de número", "Format de nombre", "Format angka", "Formato numero", "表示形式", "표시 형식", "Formato de número", "Formato de número", "Формат числа", "Sayı biçimi", "Định dạng số", "数字格式", "數值格式"]),
    ("unlocked", ["Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "ロック解除", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked", "Unlocked"]),
    ("hide_formula", ["Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "式を隠す", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula", "Hidden formula"]),
    ("hairline", ["Haarlinie", "Hairline", "Extrafina", "Trait extra-fin", "Sangat tipis", "Sottilissimo", "極細", "아주 가는 선", "Extrafina", "Extrafina", "Волосяная", "Saç teli", "Cực mảnh", "极细", "極細"]),
    ("dotted", ["Gepunktet", "Dotted", "Punteada", "Pointillés", "Titik-titik", "Punteggiato", "点線", "점선", "Pontilhada", "Pontilhada", "Точечная", "Noktalı", "Nét chấm", "点线", "點線"]),
    ("dash_dot_dot", ["Strich-Punkt-Punkt", "Dash-dot-dot", "Raya y dos puntos", "Trait-point-point", "Putus-titik-titik", "Tratto-punto-punto", "二点鎖線", "이점쇄선", "Traço-ponto-ponto", "Traço-ponto-ponto", "Штрихпунктирная с двумя точками", "Çizgi-nokta-nokta", "Nét gạch-chấm-chấm", "双点划线", "二點鏈線"]),
    ("dash_dot", ["Strich-Punkt", "Dash-dot", "Raya y punto", "Trait-point", "Putus-titik", "Tratto-punto", "一点鎖線", "일점쇄선", "Traço-ponto", "Traço-ponto", "Штрихпунктирная", "Çizgi-nokta", "Nét gạch-chấm", "点划线", "一點鏈線"]),
    ("dashed", ["Gestrichelt", "Dashed", "Discontinua", "Tirets", "Putus-putus", "Tratteggiato", "破線", "파선", "Tracejada", "Tracejada", "Штриховая", "Kesik çizgili", "Nét đứt", "虚线", "虛線"]),
    ("thin", ["Dünn", "Thin", "Fino", "Fin", "Tipis", "Sottile", "細", "가는 선", "Fino", "Fino", "Тонкая", "İnce", "Mảnh", "细", "細"]),
    ("medium_dash_dot_dot", ["Mittel Strich-Punkt-Punkt", "Medium dash-dot-dot", "Raya y dos puntos media", "Trait-point-point moyen", "Putus-titik-titik sedang", "Tratto-punto-punto medio", "中太の二点鎖線", "중간 굵기 이점쇄선", "Média traço-ponto-ponto", "Média traço-ponto-ponto", "Средняя штрихпунктирная с двумя точками", "Orta kalın çizgi-nokta-nokta", "Nét gạch-chấm-chấm vừa", "中粗双点划线", "中粗二點鏈線"]),
    ("medium_dash_dot", ["Mittel Strich-Punkt", "Medium dash-dot", "Raya y punto media", "Trait-point moyen", "Putus-titik sedang", "Tratto-punto medio", "中太の一点鎖線", "중간 굵기 일점쇄선", "Média traço-ponto", "Média traço-ponto", "Средняя штрихпунктирная", "Orta kalın çizgi-nokta", "Nét gạch-chấm vừa", "中粗点划线", "中粗一點鏈線"]),
    ("medium_dashed", ["Mittel gestrichelt", "Medium dashed", "Discontinua media", "Tirets moyens", "Putus-putus sedang", "Tratteggiato medio", "中太の破線", "중간 굵기 파선", "Média tracejada", "Média tracejada", "Средняя штриховая", "Orta kalın kesik çizgili", "Nét đứt vừa", "中粗虚线", "中粗虛線"]),
    ("medium", ["Medium", "Medium", "Medium", "Medium", "Medium", "Medium", "中", "Medium", "Medium", "Medium", "Medium", "Medium", "Medium", "Medium", "Medium"]),
    ("thick", ["Dick", "Thick", "Grueso", "Épais", "Tebal", "Spesso", "太", "굵은 선", "Grosso", "Grosso", "Толстая", "Kalın", "Đậm", "粗", "粗"]),
    ("double", ["Doppelt", "Double", "Doble", "Double", "Ganda", "Doppio", "二重", "이중선", "Duplo", "Duplo", "Двойная", "Çift", "Đôi", "双线", "雙線"]),
    ("slant_dash_dot", ["Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "斜め一点鎖線", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot"]),
    ("align_general", ["General", "General", "General", "General", "General", "General", "標準", "General", "General", "General", "General", "General", "General", "General", "General"]),
    ("left", ["Links", "Left", "Izquierda", "Gauche", "Kiri", "Sinistra", "左", "왼쪽", "Esquerda", "Esquerda", "По левому краю", "Sol", "Trái", "左", "靠左"]),
    ("center", ["Center", "Center", "Center", "Center", "Center", "Center", "中央", "Center", "Center", "Center", "Center", "Center", "Center", "Center", "Center"]),
    ("right", ["Rechts", "Right", "Derecha", "Droite", "Kanan", "Destra", "右", "오른쪽", "Direita", "Direita", "По правому краю", "Sağ", "Phải", "右", "靠右"]),
    ("justify", ["Blocksatz", "Justify", "Justificar", "Justifier", "Rata kiri-kanan", "Giustifica", "両端揃え", "양쪽 맞춤", "Justificar", "Justificar", "По ширине", "İki yana yasla", "Canh đều", "两端对齐", "左右對齊"]),
    ("center_across", ["Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "選択範囲内で中央", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection"]),
    ("distributed", ["Verteilt", "Distributed", "Distribuido", "Réparti", "Terdistribusi", "Distribuito", "均等割付", "균등 분할", "Distribuído", "Distribuído", "Распределённый", "Dağıtılmış", "Phân bố đều", "分散对齐", "分散對齊"]),
    ("top", ["Oben", "Top", "Superior", "Haut", "Atas", "In alto", "上", "위쪽", "Superior", "Superior", "По верхнему краю", "Üst", "Trên", "顶端", "靠上"]),
    ("bottom", ["Unten", "Bottom", "Inferior", "Bas", "Bawah", "In basso", "下", "아래쪽", "Inferior", "Inferior", "По нижнему краю", "Alt", "Dưới", "底端", "靠下"]),
    ("edge_top", ["Top edge", "Top edge", "Top edge", "Top edge", "Top edge", "Top edge", "上辺", "Top edge", "Top edge", "Top edge", "Top edge", "Top edge", "Top edge", "Top edge", "Top edge"]),
    ("edge_bottom", ["Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "下辺", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge", "Bottom edge"]),
    ("edge_left", ["Left edge", "Left edge", "Left edge", "Left edge", "Left edge", "Left edge", "左辺", "Left edge", "Left edge", "Left edge", "Left edge", "Left edge", "Left edge", "Left edge", "Left edge"]),
    ("edge_right", ["Right edge", "Right edge", "Right edge", "Right edge", "Right edge", "Right edge", "右辺", "Right edge", "Right edge", "Right edge", "Right edge", "Right edge", "Right edge", "Right edge", "Right edge"]),
];

/// いまの画面の言語の桁。知らない札は英語の桁
fn column() -> usize {
    let want = default_language();
    LANGS.iter().position(|l| *l == want).unwrap_or_else(en_column)
}

fn en_column() -> usize {
    LANGS.iter().position(|l| *l == "en").unwrap_or(0)
}

/// **記号 → いまの画面の言語の字。** 知らない記号は記号のまま返します
/// (黙って空にしない — テンプレートに空の見出しが並ぶと読めません)。
pub fn text(sym: &str) -> &'static str {
    match WORDS.iter().find(|(k, _)| *k == sym) {
        Some((_, t)) => t[column()],
        None => Box::leak(sym.to_string().into_boxed_str()),
    }
}

/// **その字がこの記号を指しているか。どの言語でも受けます。**
///
/// 大文字小文字と前後の空白は見ません。
pub fn is(sym: &str, text: &str) -> bool {
    let t = text.trim();
    WORDS
        .iter()
        .find(|(k, _)| *k == sym)
        .is_some_and(|(_, v)| v.iter().any(|x| x.eq_ignore_ascii_case(t) || *x == t))
}

/// **並びの中から、その字が指す記号を選ぶ。**
///
/// 同じ字が別の記号を指すことがあります(英語の `Center` は横位置にも
/// 縦位置にもある)。呼ぶ側が「この場所に来るのはこの記号のどれか」を
/// 渡すことで取り違えを防ぎます。
pub fn which(syms: &[&'static str], text: &str) -> Option<&'static str> {
    syms.iter().copied().find(|s| is(s, text))
}
