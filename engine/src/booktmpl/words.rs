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
    ("col_width", ["Spaltenbreite", "Column width", "Ancho de columna", "Largeur de colonne", "Lebar Kolom", "Larghezza colonna", "列幅", "열 너비", "Largura da coluna", "Largura da coluna", "Ширина столбца", "Sütun Genişliği", "Bề rộng cột", "列宽", "欄寬"]),
    ("row_height", ["Zeilenhöhe", "Row height", "Altura de fila", "Hauteur de ligne", "Tinggi Baris", "Altezza riga", "行の高さ", "행 높이", "Altura da Linha", "Altura da Linha", "Высота строки", "Satır Yüksekliği", "Chiều cao hàng", "行高", "列高"]),
    ("print", ["Drucken", "Print", "Imprimir", "Imprimer", "Cetak", "Stampa", "印刷", "인쇄", "Imprimir", "Imprimir", "Печать", "Yazdır", "In", "打印", "列印"]),
    ("page_break", ["Seitenumbruch", "Page break", "Salto de página", "Saut de page", "Break Halaman", "Interruzione di pagina", "改ページ", "페이지 나누기", "Quebra de página", "Quebra de página", "Разрыв страницы", "Sayfa Sonu", "Ngắt Trang", "分页符", "分頁符"]),
    ("header_footer", ["Kopf- und Fußzeile", "Header and footer", "Encabezado y pie", "En-tête et pied de page", "Header dan footer", "Intestazione e piè di pagina", "ヘッダーとフッター", "머리글과 바닥글", "Cabeçalho e rodapé", "Cabeçalho e rodapé", "Колонтитулы", "Üst bilgi ve alt bilgi", "Đầu trang và chân trang", "页眉和页脚", "頁首及頁尾"]),
    ("view", ["Anzeigen", "View", "Vista", "Affichage", "Lihat", "Visualizza", "画面", "보기", "Ver", "Visualizar", "Вид", "Görüntüle", "Xem", "视图", "檢視"]),
    ("tmpl_group", ["Gruppieren", "Group", "Agrupar", "Grouper", "Grup", "Raggruppa", "グループ化", "그룹", "Grupo", "Grupo", "Сгруппировать", "Grup", "Nhóm", "组", "分組"]),
    ("tmpl_protect", ["Schutz", "Protection", "Protección", "Protection", "Proteksi", "Protezione", "保護", "보호", "Proteção", "Proteção", "Защита", "Koruma", "Bảo vệ", "保护", "保護"]),
    ("format", ["Format", "Format", "Formato", "Format", "Format", "Formato", "書式", "서식", "Formato", "Formato", "Формат", "Biçim", "Định dạng", "格式", "格式"]),
    ("format_applied", ["Anwenden für", "Format applied to", "Aplicar a", "Appliquer à", "Terapkan ke", "Applica a", "書式の当て", "적용", "Aplicar a", "Aplicar à", "Применить к", "Başvurmak", "Format applied to", "应用于", "套用至"]),
    ("workbook", ["Arbeitsmappe", "Workbook", "Libro de trabajo", "Classeur", "Workbook", "Cartella di lavoro", "ブック", "통합 문서", "Livro de trabalho", "Pasta de trabalho", "Книга", "Not defteri", "Workbook", "工作簿", "工作簿"]),
    ("sheets", ["Blätter", "Sheets", "Hojas", "Feuilles", "Lembar kerja", "Fogli", "シート", "시트", "Folhas", "Planilhas", "Листы", "Çalışma sayfaları", "Trang tính", "工作表", "工作表"]),
    ("tmpl_column", ["Spalte", "Column", "Columna", "Colonne", "Kolom", "Colonna", "列", "열", "Coluna", "Coluna", "Столбец", "Sütun", "Cột", "列", "欄"]),
    ("width_2", ["Breite", "width", "Ancho", "la largeur", "lebar", "tutta la larghezza", "幅", "너비", "largura", "largura", "Ширина", "Genişlik", "chiều rộng", "宽度", "寬度"]),
    ("row", ["Zeile", "Row", "Fila", "Ligne", "Baris", "Riga", "行", "행", "Linha", "Linha", "Строка", "Satır", "Hàng", "行", "列"]),
    ("height", ["Höhe", "Height", "Alto", "Hauteur", "Tinggi", "Altezza", "高さ", "높이", "Altura", "Altura", "Высота", "Yükseklik", "Chiều cao", "高度", "高度"]),
    ("size", ["Größe", "Size", "Tamaño", "Taille", "Ukuran", "Dimensione", "大きさ", "크기", "Tamanho", "Tamanho", "Размер", "Boyut", "Kích thước", "大小", "大小"]),
    ("orientation", ["Ausrichtung", "Orientation", "Orientación", "Orientation", "Orientasi", "Orientamento", "向き", "방향", "Orientação", "Orientação", "Ориентация", "Yön", "Hướng", "方向", "方向"]),
    ("margins", ["Ränder", "Margins", "Márgenes", "Marges", "Margin", "Margini", "余白", "여백", "Margens", "Margens", "Поля", "Kenar boşluğu", "Lề", "页边距", "邊界"]),
    ("gridlines", ["Gitternetzlinien", "Gridlines", "Líneas de cuadrícula", "Quadrillage", "Garis Grid", "Linee griglia", "目盛線", "눈금선", "Linhas da grelha", "Linhas de grade", "Линии сетки", "Klavuz çizgileri", "Đường lưới", "网格线", "網格線"]),
    ("tmpl_zoom", ["Maßstab", "Zoom", "Ampliación", "Zoom", "Pembesaran", "Zoom", "拡大", "확대/축소", "Ampliação", "Zoom", "Масштаб", "Büyüt", "Thu phóng", "缩放", "縮放"]),
    ("scale", ["Maßstab", "Scale", "Escala", "Échelle", "Skala", "Ridimensiona", "倍率", "배율", "Redimensionar", "Redimensionar", "Масштаб", "Ölçek", "Tỷ lệ", "缩放比例", "縮放"]),
    ("fit_to_page", ["Seite anpassen", "Fit to page", "Ajustar a la página", "Ajuster à la page", "Sesuaikan Halaman", "Adatta alla pagina", "紙に収める", "페이지에 맞춤", "Ajustar à página", "Ajustar a página", "По размеру страницы", "Sayfaya Sığdır", "Vừa với trang", "调整至页面大小", "縮放至整頁"]),
    ("fit_to_width", ["Breite anpassen", "Fit to width", "Ajustar al ancho", "Ajuster à la largeur", "Sesuaikan Lebar", "Adatta alla larghezza", "横に収める", "너비에 맞춤", "Ajustar à largura", "Ajustar largura", "По ширине", "Genişliğe Sığdır", "Vừa với Chiều rộng", "调整至合适宽度", "縮放至頁面寬度"]),
    ("fit_to_height", ["Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "縦に収める", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height", "Fit to height"]),
    ("row_col_headings", ["Überschriften", "Row and column headings", "Encabezados", "Titres", "Tajuk", "Intestazioni", "行列番号", "제목", "Títulos", "Títulos", "Заголовки", "Başlıklar", "Tiêu đề", "标题", "標題"]),
    ("title_rows", ["Wiederholungszeilen", "Title rows", "Filas que repetir", "Lignes à répéter", "Baris Yang Diulang", "Righe da ripetere", "タイトル行", "행 반복", "Linhas a repetir", "Linhas a repetir", "Повторять строки", "Tekrarlanacak Satırlar", "Title rows", "需要重复的行", "要重複的列"]),
    ("title_cols", ["Wiederholungsspalten", "Title columns", "Columnas que repetir", "Colonnes à répéter", "Kolom yang Diulang", "Colonne da ripetere", "タイトル列", "열 반복", "Colunas a repetir", "Colunas a repetir", "Повторять столбцы", "Tekrarlanacak Sütunlar", "Title columns", "需要重复的列", "要重複的欄"]),
    ("position", ["Position", "Position", "Posición", "Position", "Posisi", "Posizione", "位置", "위치", "Posição", "Posição", "Положение", "Konum", "Vị trí", "位置", "位置"]),
    ("tmpl_text", ["Text", "Text", "Texto", "Texte", "Teks", "Testo", "文字", "텍스트", "Texto", "Texto", "Текст", "Metin", "Văn bản", "文本", "文字"]),
    ("freeze", ["Fensterausschnitt fixieren", "Freeze", "Congelar paneles", "Figer les volets", "Freeze Panes", "Blocca riquadri", "固定", "창 고정", "Fixar painéis", "Congelar painéis", "Закрепить области", "Parçaları Dondur", "Freeze Panes", "冻结窗格", "凍結窗格"]),
    ("formula_2", ["Formel", "Formula", "Ecuación", "Équation", "Rumus", "Formula", "数式", "수식", "Equação", "Equação", "Формула", "Denklem", "Công thức", "公式", "公式"]),
    ("rtl", ["Von rechts nach links", "Right to left", "De derecha a izquierda", "De droite à gauche", "Kanan sampai kiri", "Da destra a sinistra", "右横書き", "오른쪽에서 왼쪽으로", "Da Direita para a esquerda", "Da direita para a esquerda", "Справа налево", "Sağdan Sola", "Right To Left", "从右到左", "從右到左"]),
    ("hide", ["Ausblenden", "Hide", "Ocultar", "Masquer", "Sembunyikan", "Nascondi", "非表示", "숨기기", "Ocultar", "Ocultar", "Скрыть", "Gizle", "Ẩn", "隐藏", "隱藏"]),
    ("tab_color", ["Farbe des Tabulators", "Tab color", "Color de la pestaña", "Couleur d'onglet", "Warna Tab", "Colore scheda", "見出しの色", "탭 색상", "Cor do separador", "Cor da aba", "Цвет ярлычка", "Sekme Rengi", "Màu Tab", "标签颜色", "標籤顏色"]),
    ("default_2", ["Standard", "Default", "Predeterminado", "Par défaut", "standar", "Predefinito", "既定", "기본", "Padrão", "Padrão", "По умолчанию", "varsayılan", "Mặc định", "默认", "預設"]),
    ("default_col_width", ["Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "既定の列幅", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width", "Default column width"]),
    ("default_row_height", ["Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "既定の行の高さ", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height", "Default row height"]),
    ("kind", ["Typ", "Kind", "Tipo", "Type", "Tipe", "Tipo", "種類", "유형", "Tipo", "Tipo", "Тип", "Tip", "Loại", "类型", "類型"]),
    ("level", ["Ebene", "Level", "Nivel", "Niveau", "Tingkat", "Livello", "段", "레벨", "Nível", "Nível", "Уровень", "Seviye", "Cấp", "级别", "層級"]),
    ("tmpl_collapsed", ["Reduzieren", "Collapsed", "Contraer", "Réduire", "Kuncupkan", "Riduci", "畳む", "축소", "Recolher", "Minimizar", "Свернуть", "Daralt", "Thu gọn", "折叠", "折疊"]),
    ("allowed_actions", ["Erlaubte Aktionen", "Allowed actions", "Operaciones permitidas", "Actions permises", "Tindakan yang diizinkan", "Operazioni consentite", "許可する操作", "허용할 동작", "Operações permitidas", "Operações permitidas", "Разрешённые действия", "İzin verilen işlemler", "Thao tác được phép", "允许的操作", "允許的操作"]),
    ("name", ["Umbenennen", "Name", "Renombrar", "Renommer", "Ganti nama", "Rinomina", "名前", "이름 변경", "Mudar o nome", "Renomear", "Переименовать", "Yeniden adlandır", "Đổi tên", "重命名", "重新命名"]),
    ("item", ["Element", "Item", "Elemento", "Element", "Butir", "Elemento", "項目", "항목", "Item", "Item", "Элемент", "Öğe", "Mục", "项目", "項目"]),
    ("value", ["Wert", "Value", "Valor", "Valeur", "Nilai", "Valore", "値", "값", "Valor", "Valor", "Значение", "Değer", "Giá trị", "值", "值"]),
    ("range", ["Bereich", "Range", "Rango", "Plage", "Rentang", "Intervallo", "範囲", "범위", "Intervalo", "Intervalo", "Диапазон", "Aralık", "Phạm vi", "范围", "範圍"]),
    ("landscape_2", ["Querformat", "Landscape", "Horizontal", "paysage", "Lanskap", "Orizzontale", "横", "가로", "Horizontal", "Paisagem", "Альбомная", "Yatay", "Ngang", "横向", "橫向"]),
    ("portrait", ["Hochformat", "Portrait", "Vertical", "portrait", "Potret", "Verticale", "縦", "세로", "Vertical", "Retrato", "Книжная", "Dikey", "Dọc", "纵向", "直向"]),
    ("header", ["Kopfzeile", "header", "encabezado", "en-tête", "kepala halaman", "intestazione", "ヘッダー", "머리글", "cabeçalho", "cabeçalho", "Верхний колонтитул", "Üstbilgi", "đầu trang", "页眉", "頁首"]),
    ("footer", ["Fußzeile", "footer", "pie de página", "pied de page", "kaki halaman", "piè di pagina", "フッター", "바닥글", "rodapé", "rodapé", "Нижний колонтитул", "Altbilgi", "chân trang", "页脚", "頁尾"]),
    ("even_page", ["Gerade Seite", "Even page", "Página par", "Page paire", "Halaman genap", "Pagina pari", "偶数の頁", "짝수 페이지", "Página par", "Página par", "Четная страница", "Çift Sayfa", "Trang chẵn", "偶数页", "偶數頁"]),
    ("first_page", ["Erste Seite", "First page", "Primera página", "Première Page", "Halaman pertama", "Prima pagina", "先頭の頁", "첫 페이지", "Primeira página", "Primeira página", "Первая страница", "İlk sayfa", "First Page", "首页", "首頁"]),
    ("all_pages", ["Alle", "All", "Todo", "Tous", "Semua", "Tutti", "すべて", "모두", "Tudo", "Todos", "Все", "Tümü", "Tất cả", "全部", "全部"]),
    ("header_even", ["Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "偶数ヘッダー", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header", "Even page header"]),
    ("footer_even", ["Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "偶数フッター", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer", "Even page footer"]),
    ("header_first", ["First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "先頭ヘッダー", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header", "First page header"]),
    ("footer_first", ["First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "先頭フッター", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer", "First page footer"]),
    ("theme_colors", ["Themenfarben", "Theme colors", "Colores del tema", "Couleurs de thème", "Warna Tema", "Colori del tema", "テーマ色", "테마 색", "Cores do tema", "Cores de tema", "Цвета темы", "Tema Renkleri", "Màu theme", "主题颜色", "佈景主題色彩"]),
    ("show_r1c1", ["Z1S1-Bezugsart", "Show as R1C1", "Estilo de referencia R1C1", "Style de référence L1C1", "Referensi Style R1C1", "Stile di riferimento R1C1", "R1C1 で見せる", "R1C1 참조 양식", "Estilo de referência R1C1", "Estilo de Referência R1C1", "Стиль ссылок R1C1", "R1C1 Referans Stili", "Show as R1C1", "R1C1 参考样式", "R1C1參考樣式"]),
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
    ("edit_objects", ["Objekte bearbeiten", "Edit objects", "Editar objetos", "Modifier les objets", "Edit Objek", "Modificare oggetti", "オブジェクトの編集", "객체 편집", "Editar objetos", "Editar objetos", "Редактировать объекты", "Nesneleri düzenle", "Edit objects", "编辑对象", "編輯物件"]),
    ("bold", ["Fett", "Bold", "Negrita", "Gras", "Tebal", "Grassetto", "太字", "굵게", "Negrito", "Negrito", "Полужирный", "Kalın", "Đậm", "加粗", "粗體"]),
    ("italic", ["Kursiv", "Italic", "Cursiva", "Italique", "Miring", "Corsivo", "斜体", "기울임꼴", "Itálico", "Itálico", "Курсив", "İtalik", "Nghiêng", "倾斜", "斜體"]),
    ("underline", ["Unterstrichen", "Underline", "Subrayado", "Souligné", "Garis bawah", "Sottolineato", "下線", "밑줄", "Sublinhado", "Sublinhado", "Подчёркнутый", "Altı çizili", "Gạch dưới", "下划线", "底線"]),
    ("strikethrough", ["Durchgestrichen", "Strikethrough", "Tachado", "Barré", "Coret", "Barrato", "取り消し線", "취소선", "Rasurado", "Tachado", "Зачёркнутый", "Üstü çizili", "Gạch ngang", "删除线", "刪除線"]),
    ("subscript", ["Tiefgestellt", "Subscript", "Subíndice", "Indice", "Subskrip", "Pedice", "下付き", "아래 첨자", "Inferior à linha", "Subscrito", "Подстрочный", "Alt simge", "Chỉ số dưới", "下标", "下標"]),
    ("tmpl_borders", ["Rahmen", "Borders", "Bordes", "Bordures", "Pembatas", "Bordi", "罫線", "테두리", "Bordas", "Bordas", "Границы", "Sınırlar", "Đường viền", "边框", "邊框"]),
    ("halign", ["Horizontale Ausrichtung", "Horizontal alignment", "Alineación horizontal", "Alignement horizontal", "Perataan Horizontal", "Allineamento orizzontale", "横位置", "가로 맞춤", "Alinhamento horizontal", "Alinhamento horizontal", "Выравнивание по горизонтали", "Yatay Hizalama", "Horizontal alignment", "水平对齐", "水平對齊"]),
    ("valign", ["Vertikale Ausrichtung", "Vertical alignment", "Alineación vertical", "Alignement vertical", "Perataan vertikal", "Allineamento verticale", "縦位置", "세로 맞춤", "Alinhamento vertical", "Alinhamento vertical", "Вертикальное выравнивание", "Dikey Hizalama", "Căn chỉnh dọc", "垂直对齐", "垂直對齊"]),
    ("fill_color", ["Füllung", "Fill colour", "Relleno", "Remplissage", "Isian", "Riempimento", "塗り", "채우기", "Preenchimento", "Preenchimento", "Заливка", "Dolgu", "Màu tô", "填充", "填滿"]),
    ("fill_bg", ["Hintergrund", "Fill background", "Fondo", "Arrière-plan", "Latar belakang", "Sfondo", "塗りの地", "배경", "Fundo", "Plano de fundo", "Фон", "Arka Plan", "Nền", "背景", "背景"]),
    ("fill_pattern", ["Muster", "Fill pattern", "Patrón", "Style de motif", "Pola", "Modello", "塗りの柄", "패턴", "Padrão", "Padrão", "Узор", "Desen", "Hoa văn", "图案", "圖案"]),
    ("gradient_2", ["Verlauf", "Gradient", "Degradado", "Dégradé", "Gradasi", "Sfumatura", "グラデーション", "그라데이션", "Gradiente", "Gradiente", "Градиент", "Geçiş", "Chuyển sắc", "渐变", "漸層"]),
    ("fill_theme", ["Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "塗りのテーマ色", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color", "Fill theme color"]),
    ("font_color", ["Schriftfarbe", "Font color", "Color de la fuente", "Couleur de police", "Warna Huruf", "Colore del carattere", "文字色", "글꼴 색", "Cor do tipo de letra", "Cor da fonte", "Цвет шрифта", "Yazı Tipi Rengi", "Màu chữ", "字体颜色", "字型顏色"]),
    ("color_theme", ["Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "文字のテーマ色", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color", "Font theme color"]),
    ("tmpl_font", ["Schriftart", "Font", "Fuente", "Police", "Huruf", "Carattere", "書体", "글꼴", "Tipo de letra", "Fonte", "Шрифт", "Yazı Tipi", "Phông chữ", "字体", "字型"]),
    ("rotation_2", ["Drehung", "Rotation", "Giro", "Rotation", "Putaran", "Rotazione", "回転", "회전", "Rotação", "Rotação", "Поворот", "Döndürme", "Xoay", "旋转", "旋轉"]),
    ("wrap", ["Zeilenumbruch", "Wrap", "Ajustar", "Renvoi à la ligne", "Bungkus", "Testo a capo", "折り返して全体を表示", "줄 바꿈", "Moldar texto", "Quebrar texto", "Перенос текста", "Kaydır", "Ngắt dòng", "自动换行", "自動換列"]),
    ("shrink", ["passend schrumpfen", "Shrink to fit", "Reducir para ajustar", "Réduire pour ajuster", "Shrink untuk pas", "Riduci e adatta", "縮小", "크기에 맞게 축소", "Diminuir para ajustar", "Reduzir para caber", "Автоподбор ширины", "Sığdırmak için küçült", "Shrink to fit", "收缩以适应", "縮小以適合"]),
    ("indent_3", ["Einzug", "Indent", "Sangría", "Retrait", "Indentasi", "Rientro", "字下げ", "들여쓰기", "Avanço", "Recuo", "Отступ", "Girinti", "Thụt lề", "缩进", "縮排"]),
    ("number_format", ["Zahlenformat", "Number format", "Formato de número", "Format de nombre", "Format angka", "Formato numero", "表示形式", "표시 형식", "Formato de número", "Formato de número", "Формат числа", "Sayı biçimi", "Định dạng số", "数字格式", "數值格式"]),
    ("unlocked", ["Nicht geschützt", "Unlocked", "No protegido", "Non protégé", "Tak terlindungi", "Non protetto", "ロック解除", "보호되지 않음", "Desprotegido", "Sem proteção", "Не защищён", "Korumalı değil", "Không bảo vệ", "不被保护的", "未受保護"]),
    ("hide_formula", ["Formel ausblenden", "Hidden formula", "Ocultar fórmulas", "Masquer les formules", "Sembunyikan rumus", "Nascondi formule", "式を隠す", "수식 숨기기", "Ocultar fórmula", "Ocultar fórmula", "Скрыть формулу", "Formulü gizle", "Ẩn công thức", "隐藏公式", "隱藏公式"]),
    ("hairline", ["Haarlinie", "Hairline", "Extrafina", "Trait extra-fin", "Sangat tipis", "Sottilissimo", "極細", "아주 가는 선", "Extrafina", "Extrafina", "Волосяная", "Saç teli", "Cực mảnh", "极细", "極細"]),
    ("dotted", ["Gepunktet", "Dotted", "Punteada", "Pointillés", "Titik-titik", "Punteggiato", "点線", "점선", "Pontilhada", "Pontilhada", "Точечная", "Noktalı", "Nét chấm", "点线", "點線"]),
    ("dash_dot_dot", ["Strich-Punkt-Punkt", "Dash-dot-dot", "Raya y dos puntos", "Trait-point-point", "Putus-titik-titik", "Tratto-punto-punto", "二点鎖線", "이점쇄선", "Traço-ponto-ponto", "Traço-ponto-ponto", "Штрихпунктирная с двумя точками", "Çizgi-nokta-nokta", "Nét gạch-chấm-chấm", "双点划线", "二點鏈線"]),
    ("dash_dot", ["Strich-Punkt", "Dash-dot", "Raya y punto", "Trait-point", "Putus-titik", "Tratto-punto", "一点鎖線", "일점쇄선", "Traço-ponto", "Traço-ponto", "Штрихпунктирная", "Çizgi-nokta", "Nét gạch-chấm", "点划线", "一點鏈線"]),
    ("dashed", ["Gestrichelt", "Dashed", "Discontinua", "Tirets", "Putus-putus", "Tratteggiato", "破線", "파선", "Tracejada", "Tracejada", "Штриховая", "Kesik çizgili", "Nét đứt", "虚线", "虛線"]),
    ("thin", ["Dünn", "Thin", "Fino", "Fin", "Tipis", "Sottile", "細", "가는 선", "Fino", "Fino", "Тонкая", "İnce", "Mảnh", "细", "細"]),
    ("medium_dash_dot_dot", ["Mittel Strich-Punkt-Punkt", "Medium dash-dot-dot", "Raya y dos puntos media", "Trait-point-point moyen", "Putus-titik-titik sedang", "Tratto-punto-punto medio", "中太の二点鎖線", "중간 굵기 이점쇄선", "Média traço-ponto-ponto", "Média traço-ponto-ponto", "Средняя штрихпунктирная с двумя точками", "Orta kalın çizgi-nokta-nokta", "Nét gạch-chấm-chấm vừa", "中粗双点划线", "中粗二點鏈線"]),
    ("medium_dash_dot", ["Mittel Strich-Punkt", "Medium dash-dot", "Raya y punto media", "Trait-point moyen", "Putus-titik sedang", "Tratto-punto medio", "中太の一点鎖線", "중간 굵기 일점쇄선", "Média traço-ponto", "Média traço-ponto", "Средняя штрихпунктирная", "Orta kalın çizgi-nokta", "Nét gạch-chấm vừa", "中粗点划线", "中粗一點鏈線"]),
    ("medium_dashed", ["Mittel gestrichelt", "Medium dashed", "Discontinua media", "Tirets moyens", "Putus-putus sedang", "Tratteggiato medio", "中太の破線", "중간 굵기 파선", "Média tracejada", "Média tracejada", "Средняя штриховая", "Orta kalın kesik çizgili", "Nét đứt vừa", "中粗虚线", "中粗虛線"]),
    ("medium", ["Mittel", "Medium", "media", "Moyen", "sedang", "medio", "中", "중간 굵기", "Média", "Média", "Средняя", "Orta kalın", "Nét", "中粗", "中粗"]),
    ("thick", ["Dick", "Thick", "Grueso", "Épais", "Tebal", "Spesso", "太", "굵은 선", "Grosso", "Grosso", "Толстая", "Kalın", "Đậm", "粗", "粗"]),
    ("double", ["Doppelt", "Double", "Doble", "Double", "Ganda", "Doppio", "二重", "이중선", "Duplo", "Duplo", "Двойная", "Çift", "Đôi", "双线", "雙線"]),
    ("diagonal", ["Diagonal", "Diagonal", "Diagonal", "Diagonale", "Diagonal", "Diagonale", "斜め", "대각선", "Diagonal", "Diagonal", "По диагонали", "Köşegen", "Diagonal", "对角线", "對角線"]),
    ("selection", ["Auswahl", "Selection", "Selección", "Sélection", "Pilihan", "Selezione", "選択範囲", "선택", "Seleção", "Seleção", "Выделенный фрагмент", "Seçim", "Lựa chọn", "选择", "選擇"]),
    ("slant_dash_dot", ["Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "斜め一点鎖線", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot", "Slanted dash dot"]),
    ("align_general", ["Allgemein", "General", "General", "Général", "Umum", "Generale", "標準", "일반", "Geral", "Geral", "Общие", "Genel", "Tổng quát", "常规", "一般"]),
    ("left", ["Links", "Left", "Izquierda", "Gauche", "Kiri", "Sinistra", "左", "왼쪽", "Esquerda", "Esquerda", "По левому краю", "Sol", "Trái", "左", "靠左"]),
    ("center", ["Zentriert", "Center", "Centro", "Au centre", "Tengah", "Al centro", "中央", "가운데", "Centro", "Centro", "По центру", "Orta", "Trung tâm", "居中", "置中"]),
    ("right", ["Rechts", "Right", "Derecha", "Droite", "Kanan", "Destra", "右", "오른쪽", "Direita", "Direita", "По правому краю", "Sağ", "Phải", "右", "靠右"]),
    ("justify", ["Blocksatz", "Justify", "Justificar", "Justifier", "Rata kiri-kanan", "Giustifica", "両端揃え", "양쪽 맞춤", "Justificar", "Justificar", "По ширине", "İki yana yasla", "Canh đều", "两端对齐", "左右對齊"]),
    ("center_across", ["Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "選択範囲内で中央", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection", "Center across selection"]),
    ("distributed", ["Verteilt", "Distributed", "Distribuido", "Réparti", "Terdistribusi", "Distribuito", "均等割付", "균등 분할", "Distribuído", "Distribuído", "Распределённый", "Dağıtılmış", "Phân bố đều", "分散对齐", "分散對齊"]),
    ("top", ["Oben", "Top", "Superior", "Haut", "Atas", "In alto", "上", "위쪽", "Superior", "Superior", "По верхнему краю", "Üst", "Trên", "顶端", "靠上"]),
    ("bottom", ["Unten", "Bottom", "Inferior", "Bas", "Bawah", "In basso", "下", "아래쪽", "Inferior", "Inferior", "По нижнему краю", "Alt", "Dưới", "底端", "靠下"]),
    ("edge_top", ["Rahmenlinie oben", "Top edge", "Borde superior", "Bordure supérieure", "Pembatas Atas", "Bordo superiore", "上辺", "위쪽 테두리", "Contorno superior", "Borda superior", "Верхняя граница", "Üst Sınır", "Top edge", "上边框", "上邊框"]),
    ("edge_bottom", ["Rahmenlinie unten", "Bottom edge", "Borde inferior", "Bordure inférieure", "Batas Bawah", "Bordo inferiore", "下辺", "아래쪽 테두리", "Contorno inferior", "Borda inferior", "Нижняя граница", "Alt Sınır", "Viền dưới", "底部边框", "底部邊框"]),
    ("edge_left", ["Rahmenlinie links", "Left edge", "Borde izquierdo", "Bordure gauche", "Pembatas Kiri", "Bordo sinistro", "左辺", "왼쪽 테두리", "Contorno esquerdo", "Limite esquerdo", "Левая граница", "Sol Sınır", "Left edge", "左边框", "左邊框"]),
    ("edge_right", ["Rahmenlinie rechts", "Right edge", "Borde derecho", "Bordure droite", "Pembatas Kanan", "Bordo destro", "右辺", "오른쪽 테두리", "Contorno direito", "Borda direita", "Правая граница", "Sağ Sınır", "Right edge", "右邊框", "右邊界"]),
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
///
/// **画面の言語を先に見ます。** 同じ字が言語をまたいで別の意味を
/// 持つことがあるためです。台湾の中国語では `列` が行のことで、
/// 日本語の `列` は列のことです。どの言語でも受ける作りのままだと、
/// 日本語で書いたテンプレートの `列` が行として読まれます
/// (2026-08-27 に、13 言語の訳を入れて往復の試験が落ちて分かりました)。
/// 画面の言語で当たらなかったときだけ、他の言語も見ます。
pub fn which(syms: &[&'static str], text: &str) -> Option<&'static str> {
    let t = text.trim();
    let c = column();
    let ima = |s: &&str| {
        WORDS
            .iter()
            .find(|(k, _)| k == s)
            .is_some_and(|(_, v)| v[c].eq_ignore_ascii_case(t) || v[c] == t)
    };
    syms.iter().copied().find(|s| ima(s)).or_else(|| syms.iter().copied().find(|s| is(s, text)))
}
