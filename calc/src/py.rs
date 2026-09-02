//! main.rs からの純移動(2026-08-06 の分割)。挙動は変えない。

use crate::*;

/// PY の引数を Python の書き方(リテラル)にする。
/// 自前の関数の表(モジュール名 → (関数名, 説明) の並び)。
type OwnFuncTable = HashMap<String, Vec<(String, String)>>;
/// シートごとの呼び出し(シート番号, (置き場, 関数名, 引数) の並び)。
type CallsPerSheet = (usize, Vec<(String, String, Vec<book::calc::PyArg>)>);
/// シートごとの式の呼び出し(シート番号, (置き場, 式, 関数名, 引数) の並び)。
type FormulaCallsPerSheet = (usize, Vec<(String, String, String, Vec<book::calc::PyArg>)>);
/// 差し込みの1仕事(置き場, (場所, 中身) の並び, 右下)。
type MergeJob = (Pos, Vec<(Pos, String)>, Pos);

pub(crate) fn py_literal(v: &book::Value) -> String {
    match v {
        book::Value::Number(n) => format!("{n}"),
        book::Value::Bool(b) => (if *b { "True" } else { "False" }).into(),
        book::Value::Empty => "none".into(),
        v => format!("{:?}", v.display()), // Rust の {:?} は Python でも読める逃がし
    }
}

/// @計算 の台本。plugins の .py を**それぞれ別のモジュールとして**読み込み、
/// 各 PY セルを評価して区切りの印(\x1c セル / \x1e 行 / \x1f 欄)で吐く。
/// mods は (モジュール名, .py の中身)、calls は (セルA1, モジュール名, 関数名, 引数)。
pub(crate) fn build_udf_script(
    mods: &[(String, String)],
    calls: &[(String, String, String, Vec<book::calc::PyArg>)],
    out_path: &std::path::Path,
) -> String {
    let mut defs = String::new();
    for (name, src) in mods {
        // 中身は文字列として渡す(名前が Python の識別子でなくてもよい)
        defs.push_str(&format!("_jo_mod({name:?}, {src:?})\n"));
    }
    let mut body = String::new();
    for (cell, module, fname, args) in calls {
        let mut lit_args = Vec::new();
        for a in args {
            match a {
                book::calc::PyArg::One(v) => lit_args.push(py_literal(v)),
                book::calc::PyArg::Rect(cols, vs) => {
                    let cols = (*cols as usize).max(1);
                    let rows: Vec<String> = vs
                        .chunks(cols)
                        .map(|row| {
                            format!(
                                "[{}]",
                                row.iter().map(py_literal).collect::<Vec<_>>().join(",")
                            )
                        })
                        .collect();
                    lit_args.push(format!("[{}]", rows.join(",")));
                }
            }
        }
        body.push_str(&format!(
            "_jo_emit({cell:?}, _jo_fn({module:?}, {fname:?})({args}))\n",
            cell = cell,
            module = module,
            fname = fname,
            args = lit_args.join(", ")
        ));
    }
    format!(
        concat!(
            "# aiseed calc の PY(UDF)評価。関数の定義は plugins の .py にある\n",
            "# (ブックはコードを運ばない — データとプログラムは別のファイル)\n",
            "import types\n",
            "_jo_mods = {{}}\n",
            "def _jo_mod(name, src):\n",
            "    m = types.ModuleType(name)\n",
            "    m.__file__ = name + '.py'\n",
            "    exec(compile(src, m.__file__, 'exec'), m.__dict__)\n",
            "    _jo_mods[name] = m\n",
            "def _jo_fn(module, fname):\n",
            "    f = getattr(_jo_mods[module], fname, None)\n",
            "    if f is None:\n",
            "        raise NameError(module + '.py に ' + fname + ' がありません')\n",
            "    return f\n",
            "{defs}\n",
            "_jo_out = []\n",
            "def _jo_emit(cell, r):\n",
            "    if not isinstance(r, (list, tuple)):\n",
            "        r = [[r]]\n",
            "    elif r and not isinstance(r[0], (list, tuple)):\n",
            "        r = [[v] for v in r]  # 1次元は縦に広げる\n",
            "    rows = ['\\x1f'.join('' if v is None else str(v) for v in row) for row in r]\n",
            "    _jo_out.append(cell + '\\x1e' + '\\x1e'.join(rows))\n",
            "{body}\n",
            "open({out:?}, 'w', encoding='utf-8').write('\\x1c'.join(_jo_out))\n"
        ),
        defs = defs,
        body = body,
        out = out_path.to_string_lossy()
    )
}

/// 台本の出力を (セル, 行×欄の文字) に戻す。
pub(crate) fn parse_udf_output(raw: &str) -> Vec<(Pos, Vec<Vec<String>>)> {
    raw.split('\u{1c}')
        .filter_map(|rec| {
            let mut it = rec.split('\u{1e}');
            let cell = Pos::parse(it.next()?)?;
            let rows: Vec<Vec<String>> = it
                .map(|r| r.split('\u{1f}').map(|v| v.to_string()).collect())
                .collect();
            (!rows.is_empty()).then_some((cell, rows))
        })
        .collect()
}

/// PY の結果をシートへ。アンカーのセルは**式を保ったまま**値を差し替え、
/// 2次元はスピル(右下へ展開)。他人のデータを潰しそうなら #SPILL! で止まる。
/// 返すのは (新しいスピルの台帳, 適用した数, 衝突した数)。
pub(crate) fn apply_py_results(
    sh: &mut book::Sheet,
    results: &[(Pos, Vec<Vec<String>>)],
    prev: &std::collections::HashMap<Pos, (u32, u32)>,
) -> (std::collections::HashMap<Pos, (u32, u32)>, usize, usize) {
    // 前回のスピル面(アンカー以外)をまず消す(小さくなったとき古い値を残さない)
    for (anchor, (rows, cols)) in prev {
        for dr in 0..*rows {
            for dc in 0..*cols {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let p = Pos::new(anchor.row + dr, anchor.col + dc);
                if let Some(c) = sh.cells.get_mut(&p) {
                    if c.formula.is_none() {
                        c.value = book::Value::Empty;
                    }
                }
            }
        }
    }
    let mut spills = std::collections::HashMap::new();
    let (mut applied, mut conflicts) = (0usize, 0usize);
    for (anchor, rows) in results {
        let (nr, nc) = (rows.len() as u32, rows.iter().map(|r| r.len()).max().unwrap_or(1) as u32);
        // 衝突検査(アンカー以外に、中身か式のあるセルが居ないか)
        let mut blocked = false;
        for dr in 0..nr {
            for dc in 0..nc {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let p = Pos::new(anchor.row + dr, anchor.col + dc);
                if let Some(c) = sh.cells.get(&p) {
                    let was_prev_spill = prev
                        .get(anchor)
                        .is_some_and(|(pr, pc)| dr < *pr && dc < *pc);
                    if c.formula.is_some() || (!c.value.is_empty() && !was_prev_spill) {
                        blocked = true;
                    }
                }
            }
        }
        let put = |sh: &mut book::Sheet, p: Pos, text: &str| {
            let fmt = sh.get(p).map(|c| c.fmt.clone()).unwrap_or_default();
            let formula = sh.get(p).and_then(|c| c.formula.clone());
            let value = if text.is_empty() {
                book::Value::Empty
            } else if let Ok(n) = text.parse::<f64>() {
                book::Value::Number(n)
            } else {
                book::Value::Text(text.to_string())
            };
            sh.set(p, book::Cell { formula, value, fmt });
        };
        if blocked {
            let fmt = sh.get(*anchor).map(|c| c.fmt.clone()).unwrap_or_default();
            let formula = sh.get(*anchor).and_then(|c| c.formula.clone());
            sh.set(
                *anchor,
                book::Cell {
                    formula,
                    value: book::Value::Error("#SPILL!".into()),
                    fmt,
                },
            );
            conflicts += 1;
            continue;
        }
        for (dr, row) in rows.iter().enumerate() {
            for (dc, text) in row.iter().enumerate() {
                put(sh, Pos::new(anchor.row + dr as u32, anchor.col + dc as u32), text);
            }
        }
        if nr > 1 || nc > 1 {
            spills.insert(*anchor, (nr, nc));
        }
        applied += 1;
    }
    (spills, applied, conflicts)
}

pub(crate) use pyrun::{
    cage_work_dir, caged_python, find_python, CHART_PY, CSV_PY, EQ_PY, SOLVER_PY,
    TEXTART_PY,
};

/// pyrun の答えを従来の文言(訳つき)に写す包み。呼び出し側を変えない
pub(crate) fn run_with_timeout(
    cmd: &mut std::process::Command,
    secs: u64,
) -> Result<(bool, String, String), String> {
    pyrun::run_with_timeout(cmd, secs).map_err(|e| match e {
        pyrun::RunErr::Spawn(e) => format!("Python が起動できません: {e}"),
        pyrun::RunErr::Timeout(s) => {
            ui::tf!("stopped_after_seconds_endless", s).to_string()
        }
        pyrun::RunErr::Wait(e) => e,
    })
}

/// **1回ごとの作業場**(2026-08-21)。
///
/// 前は `jo-<種類>-<プロセス番号>` の1つを使い回していました。**同時に
/// 2つ走ると同じファイルを取り合います** — 「すべて更新」は開いている
/// ピボットを全部いっぺんに走らせるので、片方の答えがもう片方に入り、
/// *黙って値が壊れます*。試験を1本足したときに実際に出ました
/// (筆記具の合計 150 のはずが、隣の試験の 300 になった)。
///
/// 番号は増えるだけの数え札です。時刻や乱数は使いません — 同じ入力から
/// 同じ物が出る方が、後から追いかけやすいためです。
pub(crate) fn workdir(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static INDEX_OF: AtomicU64 = AtomicU64::new(0);
    let n = INDEX_OF.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("jo-{name}-{}-{n}", std::process::id()))
}

#[derive(Clone)]
pub(crate) struct ImportPend {
    pub path: std::path::PathBuf,
    /// IMPORT_ENCS の添字
    pub enc: usize,
    /// IMPORT_DELIMS の添字
    pub delim: usize,
    /// 「その他」の区切り(1文字)
    pub custom: String,
    pub dest: Pos,
    /// いまの設定で読んだ全行(取り込むときはこれを流し込む)
    pub grid: Vec<Vec<String>>,
    /// Python が実際に使った(文字コード, 区切り)— 自動のときの報告
    pub used: (String, String),
    /// **PDF から取った表**(ページ番号, 取り方, セル)。空 = PDF ではない。
    /// 取り方は「罫線」か「文字の位置」で、**そのまま画面に出します** —
    /// 推し量って取った表を、正確に取れた表と同じ顔で出さないためです
    pub pdf: Vec<(u32, String, Vec<Vec<String>>)>,
    /// いま見ている表(`pdf` の添字)
    pub pdf_at: usize,
}

/// 文字コードの選択肢(鍵, 見せる名前, Python に渡す名前)。
/// 符号化の名前(UTF-8 など)は**固有名詞なので訳さない** — 鍵と見出しが同じ
pub(crate) fn import_encs() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        crate::util::row(ui::item!("automatic"), "auto"),
        ("UTF-8", "UTF-8", "utf-8-sig"),
        ("Shift_JIS(CP932)", "Shift_JIS(CP932)", "cp932"),
        ("Latin-1", "Latin-1", "latin-1"),
    ]
}

/// 区切りの選択肢(鍵, 見せる名前, 実体)。「その他」はパネルで1文字を聞く。
/// **実体は訳さない** — `その他` は分岐の合図として読まれる
pub(crate) fn import_delims() -> Vec<(&'static str, &'static str, &'static str)> {
    use crate::util::row;
    vec![
        row(ui::item!("automatic"), "auto"),
        row(ui::item!("comma"), ","),
        row(ui::item!("tab"), "\t"),
        row(ui::item!("semicolon"), ";"),
        row(ui::item!("colon"), ":"),
        row(ui::item!("space"), " "),
        row(ui::item!("other"), "その他"),
    ]
}

/// サンドボックスの中で走る Command を組む。組めない機械(bwrap が無い・
/// macOS・Windows)では普通の Python を返す — 自分で置いたマクロは、サンドボックスが
/// 無くても今までどおり走る(他所から来たコードとは扱いが違う)。
///
/// サンドボックスの中に見せる物:
/// * 読み取り専用 — `.venv`(綴りの物と利用者の物)・`pip install -e` の実体・
///   Python 本体の置き場・`extra_ro`(plugins / ribbon / funcs の置き場)
/// * 読み書き — 作業場 `dir` と、**calc のソケットの置き場**。表のマクロは
///   開いているブックをソケット越しに書くので、ここが見えないと
///   `cannot connect to officework` で止まる
pub(crate) fn caged_or_plain(
    py: &std::path::Path,
    dir: &std::path::Path,
    extra_ro: &[std::path::PathBuf],
) -> std::process::Command {
    // bwrap があっても起動できない機械(AppArmor がユーザー名前空間を
    // 禁じている)では、普通の Python で走らせる
    if !pyrun::cage_works() {
        return std::process::Command::new(py);
    }
    let mut ro: Vec<std::path::PathBuf> = Vec::new();
    for venv in [std::fs::canonicalize(".venv").ok(), Some(pyrun::venv_dir())]
        .into_iter()
        .flatten()
    {
        ro.extend(pyrun::editable_paths(&venv));
        ro.push(venv);
    }
    // 見つけた Python の置き場(venv の根 = bin の親)。ホームの下の venv は
    // 隠れるので、名指しで見せる
    if let Some(root) = py.parent().and_then(|b| b.parent()) {
        ro.push(root.to_path_buf());
    }
    if let Some(pp) = std::env::var_os("PYTHONPATH") {
        ro.extend(std::env::split_paths(&pp).filter_map(|p| std::fs::canonicalize(p).ok()));
    }
    ro.extend(extra_ro.iter().cloned());
    // ソケットは unix だけ(Windows にはこのサンドボックスも無い)
    #[cfg(unix)]
    let rw: Vec<std::path::PathBuf> = ops::sock_path("calc")
        .parent()
        .map(|d| vec![d.to_path_buf()])
        .unwrap_or_default();
    #[cfg(not(unix))]
    let rw: Vec<std::path::PathBuf> = Vec::new();
    match pyrun::caged_python_open(pyrun::cage_kind(), py, dir, &ro, &rw, false) {
        Some(c) => c,
        None => std::process::Command::new(py),
    }
}

/// プラグイン(.py)の置き場。**正は pyrun**(writer と共有)。
/// 呼び出し側を変えないための包み
pub(crate) fn plugins_dir() -> PathBuf {
    pyrun::plugins_dir()
}

pub(crate) use pyrun::{def_names, plugin_outline};

/// UDF の登録簿。**大文字にした関数名** → その名前を持つ (モジュール, 実際の名前)。
/// 字句解析が ASCII を大文字にするので、こちらも大文字で引く(日本語はそのまま)。
static UDF_MAP: std::sync::RwLock<Option<OwnFuncTable>> =
    std::sync::RwLock::new(None);

/// plugins を読み直して UDF の登録簿を作り、sheet に名前を渡す。
/// 返りは**組み込み関数と名前がぶつかって見送ったもの**(黙って握り潰さない)。
/// 中身は実行しない — `def` の行を数えるだけ。
pub(crate) fn refresh_udfs() -> Vec<String> {
    let mut map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut clash: Vec<String> = Vec::new();
    // **式から呼べるのは funcs の .py だけ**(2026-08-16)。前は plugins を
    // 舐めていて、マクロの補助関数まで表の関数になっていた
    for m in pyrun::modules_in(&pyrun::funcs_dir()) {
        let Ok(src) = std::fs::read_to_string(pyrun::funcs_dir().join(format!("{m}.py"))) else {
            continue;
        };
        for f in def_names(&src) {
            let up = f.to_ascii_uppercase();
            // 組み込みの関数名は譲らない(SUM を上書きされたら帳票が壊れる)
            if up == "PY" || crate::funcs::FUNCS.iter().any(|x| x.name == up) {
                clash.push(format!("{m}.{f}"));
                continue;
            }
            map.entry(up).or_default().push((m.clone(), f));
        }
    }
    book::calc::set_udf_names(map.keys().cloned());
    if let Ok(mut g) = UDF_MAP.write() {
        *g = Some(map);
    }
    clash
}

/// plugins を最後に見たときの姿。**置き場の時刻だけでは足りない** —
/// 中の .py を書き換えても置き場の時刻は動かないので(項目の出入りでしか
/// 動かない)、**1つ1つの名前・大きさ・時刻**を見る。
static PLUGINS_SEEN: std::sync::Mutex<Option<Vec<(String, u64, std::time::SystemTime)>>> =
    std::sync::Mutex::new(None);

/// plugins が変わっていれば登録簿を作り直す。返りは作り直したか。
pub(crate) fn refresh_udfs_if_changed() -> bool {
    let now = pyrun::shape_in(&pyrun::funcs_dir());
    let Ok(mut last) = PLUGINS_SEEN.lock() else { return false };
    if last.as_ref() == Some(&now) {
        return false;
    }
    *last = Some(now);
    drop(last);
    refresh_udfs();
    true
}

/// UDF の見張り。200ms ごとに (1) plugins が変わっていないか
/// (2) 引数が変わっていないか を見て、要れば裏で計算し直す。
pub(crate) fn start_udf_watch(view: gpui::Entity<Calc>, cx: &mut gpui::App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(200))
                .await;
            view.update(cx, |calc, cx| {
                // plugins が変われば、式の見え方(どれが UDF か)も、関数の
                // 中身も変わる。**指紋を捨てて計算し直させる** — 引数が同じ
                // ままでも、関数の中身が変わっていれば答えは変わるため
                if refresh_udfs_if_changed() {
                    book::calc::recalc_all(&mut calc.book);
                    calc.udf_stamp.clear();
                    cx.notify();
                }
                // リボンに置いたマクロも見る。**別の置き場なので別に見る** —
                // .py を足したのに次に起こすまでボタンが出ない、では手が止まる
                if ui::ribbon::refresh_user_cmds() {
                    cx.notify();
                }
                calc.udf_tick(cx);
            });
        }
    })
    .detach();
}

/// 式に書かれた名前を plugins の .py に結ぶ。返すのは (モジュール名, 関数名)。
/// 普段は関数名だけでよい。同じ名前が2つの .py にあるときだけ
/// "モジュール.関数" と書いて選ぶ — 無い・複数あるときは、そう言う
/// (黙って選ばない)。
pub(crate) fn resolve_udf(name: &str) -> Result<(String, String), String> {
    if let Some((m, f)) = name.rsplit_once('.') {
        return if pyrun::funcs_dir().join(format!("{m}.py")).exists() {
            Ok((m.to_string(), f.to_string()))
        } else {
            Err(format!("{m}.py がありません"))
        };
    }
    let hits: Vec<(String, String)> = UDF_MAP
        .read()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(&name.to_ascii_uppercase()).cloned()))
        .unwrap_or_default();
    match hits.len() {
        0 => Err(format!("「{name}」の定義が funcs にありません")),
        1 => Ok(hits[0].clone()),
        _ => Err(format!(
            "「{name}」が {} にあります — {}.{name} のようにモジュール名を付けてください",
            hits.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>().join(" と "),
            hits[0].0
        )),
    }
}

impl Calc {

    /// .py を開く(無ければ下書きを置く)。
    ///
    /// **置き場は中身で決める**(2026-08-16): funcs に在ればそちら、
    /// plugins に在ればそちら、どちらにも無ければ **funcs**(新しく書く物は
    /// たいてい式から呼ぶ関数で、保存したら見張りが拾って計算し直る)
    pub(crate) fn open_py_edit(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.status = ui::t!("write_edit_name_example").into();
            return;
        }
        let dir = if plugins_dir().join(format!("{name}.py")).exists()
            && !pyrun::funcs_dir().join(format!("{name}.py")).exists()
        {
            plugins_dir()
        } else {
            pyrun::funcs_dir()
        };
        let path = dir.join(format!("{name}.py"));
        let text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => ui::pyedit::skeleton(name),
        };
        self.py_edit = Some(ui::pyedit::PyEdit {
            name: name.to_string(),
            dir,
            ed: Editor::new(&text),
            top: 0,
            saved: text.clone(),
        });
        // 先頭に置く(開いた瞬間に全部選ばれていると、1打で消える)
        if let Some(p) = &mut self.py_edit {
            p.ed.move_to(0, false);
        }
        self.status =
            ui::tf!("opened_ctrl_s_saves", path.display().to_string())
                .into();
    }

    /// 書き出す。**保存した時点で見張りが気づき、シートが計算し直る。**
    pub(crate) fn save_py_edit(&mut self) {
        let Some(p) = &mut self.py_edit else { return };
        // **開いた置き場へ書き戻す**(funcs か plugins か)
        let dir = p.dir.clone();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = ui::tf!("cannot_create_folder", e.to_string()).into();
            return;
        }
        let path = dir.join(format!("{}.py", p.name));
        match std::fs::write(&path, p.ed.text()) {
            Ok(_) => {
                p.saved = p.ed.text().to_string();
                let n = p.name.clone();
                self.status = ui::tf!("saved_py_cell_functions", n).into();
            }
            Err(e) => self.status = ui::tf!("cant_write", e.to_string()).into(),
        }
    }

    /// 閉じる。**書きかけがあれば一度断る**(黙って捨てない)。
    /// もう一度 Esc を押すと捨てて閉じる。
    pub(crate) fn close_py_edit(&mut self) {
        let Some(p) = &self.py_edit else { return };
        if p.dirty() && !self.py_edit_ask {
            self.py_edit_ask = true;
            self.status =
                ui::t!("there_unsaved_edits_ctrl").into();
            return;
        }
        self.py_edit = None;
        self.py_edit_ask = false;
        self.status = ui::t!("closed").into();
    }

    /// 選んだ範囲を matplotlib で棒グラフにして、シートに浮かべる。
    /// 1列目が項目名、残りの列が系列(先頭行が文字なら系列名)。
    /// Python は別のスレッドで回す(メインスレッドを塞がない — ダイアログと同じ作法)。
    pub(crate) fn insert_chart(&mut self, a: Pos, b: Pos, cx: &mut Context<Self>) {
        self.insert_chart_kind(a, b, "bar", cx);
    }

    /// 推奨グラフ。選んだ範囲の形を見て、合う種類を一覧に並べます。
    /// 選ぶと [`Self::insert_chart_kind`] がその種類で描きます
    /// (手引き `docs/ja/commands/挿入/推奨チャートを挿入.adoc`)。
    pub(crate) fn recommend_chart(&mut self) {
        if self.anchor.is_none() {
            self.status = ui::t!("select_range_chart_first").into();
            return;
        }
        let (a, b) = self.sel_rect();
        let shape = self.range_shape(a, b);
        if shape.points == 0 {
            self.status = ui::t!("select_range_chart_first").into();
            return;
        }
        let at = self.pop_anchor();
        let items: Vec<(String, String)> = recommended_kinds(shape)
            .iter()
            .map(|k| {
                let (key, label) = chart_kind_item(k);
                (key.to_string(), label.to_string())
            })
            .collect();
        self.pick_note = Some(ui::t!("chart_types_fit_selection").into());
        self.pick_kind = "chart-kind-pick";
        self.pick = Some((items, at));
    }

    /// 推奨グラフの一覧で選んだ物を描く。`v` は一覧の鍵(chart_column など)。
    /// Enter で確定したときは見出しの字が来るので、見出しでも引き当てる
    pub(crate) fn insert_chart_picked(&mut self, v: &str, cx: &mut Context<Self>) {
        let Some(kind) = CHART_KINDS
            .iter()
            .find(|(k, key)| *key == v || chart_kind_item(k).1.as_ref() == v)
            .map(|(k, _)| *k)
        else {
            return;
        };
        if self.anchor.is_none() {
            self.status = ui::t!("select_range_chart_first").into();
            return;
        }
        let (a, b) = self.sel_rect();
        self.insert_chart_kind(a, b, kind, cx);
    }

    /// 選んだ範囲の形を測る(推奨グラフの判断の材料)。
    /// 1列目は項目名、2列目からが系列。先頭行に文字があれば見出し行
    pub(crate) fn range_shape(&self, a: Pos, b: Pos) -> RangeShape {
        let sh = self.sheet();
        let header = (a.col + 1..=b.col).any(|c| {
            matches!(sh.get(Pos::new(a.row, c)).map(|x| &x.value), Some(book::Value::Text(_)))
        });
        let r0 = if header { a.row + 1 } else { a.row };
        let rows: Vec<u32> = (r0..=b.row).collect();
        let first: Vec<Option<&book::Cell>> =
            rows.iter().map(|r| sh.get(Pos::new(*r, a.col))).collect();
        let first_col_numeric = !first.is_empty()
            && first.iter().all(|c| matches!(c.map(|x| &x.value), Some(book::Value::Number(_))));
        // 日付の表示形式か、4桁の年か、「4月」のような月の字なら時系列
        let first_col_time = !first.is_empty()
            && first.iter().all(|c| match c {
                Some(x) => {
                    let t = x.value.display();
                    x.fmt.number_format.as_deref().is_some_and(|f| {
                        f.contains('y') || f.contains('m') || f.contains('d')
                    }) || looks_like_time_label(&t)
                }
                None => false,
            });
        let mut has_negative = false;
        for c in a.col + 1..=b.col {
            for r in &rows {
                if let Some(x) = sh.get(Pos::new(*r, c)) {
                    if matches!(x.value, book::Value::Number(n) if n < 0.0) {
                        has_negative = true;
                    }
                }
            }
        }
        RangeShape {
            points: rows.len(),
            series: (b.col.saturating_sub(a.col)) as usize,
            first_col_numeric,
            first_col_time,
            has_negative,
        }
    }

    /// 種類を指して差し込む。予測シートは折れ線を使います
    pub(crate) fn insert_chart_kind(
        &mut self,
        a: Pos,
        b: Pos,
        kind: &'static str,
        cx: &mut Context<Self>,
    ) {
        let sh = self.sheet();
        // 先頭行が見出しか(項目列以外に文字があるか)
        let header = (a.col + 1..=b.col).any(|c| {
            matches!(
                sh.get(Pos::new(a.row, c)).map(|x| &x.value),
                Some(book::Value::Text(_))
            )
        });
        let r0 = if header { a.row + 1 } else { a.row };
        let labels: Vec<String> = (r0..=b.row)
            .map(|r| {
                let v = sh.get(Pos::new(r, a.col)).map(|x| x.value.display()).unwrap_or_default();
                if v.is_empty() { (r + 1).to_string() } else { v }
            })
            .collect();
        let mut series = Vec::new();
        for c in a.col + 1..=b.col {
            let name = if header {
                sh.get(Pos::new(a.row, c)).map(|x| x.value.display()).unwrap_or_default()
            } else {
                col_name(c)
            };
            let values: Vec<Option<f64>> = (r0..=b.row)
                .map(|r| match sh.get(Pos::new(r, c)) {
                    // **空は空のまま渡す**(折れ線が谷にならないように)。
                    // 棒のときは 0 として描かれます
                    None => None,
                    Some(x) if matches!(x.value, book::Value::Empty) => None,
                    Some(x) => Some(x.value.as_number()),
                })
                .collect();
            series.push((name, values));
        }
        if labels.is_empty() || series.is_empty() {
            self.status = ui::t!("select_range_chart_first").into();
            return;
        }
        // JSON は手で組む(依存を増やさない。文字列は最小の逃がし)
        let esc = |t: &str| t.replace('\\', "\\\\").replace('"', "\\\"");
        let dir = workdir("chart");
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("chart.png");
        let font = kumihan::font::for_document(None)
            .ok()
            .map(|(fam, _)| fam.path.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut json = String::from("{\"labels\":[");
        json.push_str(&labels.iter().map(|l| format!("\"{}\"", esc(l))).collect::<Vec<_>>().join(","));
        json.push_str("],\"series\":[");
        json.push_str(
            &series
                .iter()
                .map(|(n, vs)| {
                    format!(
                        "{{\"name\":\"{}\",\"values\":[{}]}}",
                        esc(n),
                        vs.iter()
                            .map(|v: &Option<f64>| match v {
                                Some(x) => x.to_string(),
                                None => "null".into(),
                            })
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join(","),
        );
        json.push_str(&format!(
            "],\"kind\":\"{}\",\"font\":\"{}\",\"out\":\"{}\"}}",
            kind,
            esc(&font),
            esc(&out.to_string_lossy())
        ));
        // 置き場が指してあればそこへ(ピボットグラフ)。無ければ範囲の右隣
        let at = self.chart_dest.take().unwrap_or_else(|| Pos::new(a.row, b.col + 1));
        self.status = ui::t!("drawing_chart").into();
        let task = cx.background_executor().spawn(async move {
            let json_path = dir.join("chart.json");
            let py_path = dir.join("chart.py");
            std::fs::write(&json_path, json).map_err(|e| e.to_string())?;
            std::fs::write(&py_path, CHART_PY).map_err(|e| e.to_string())?;
            let o = std::process::Command::new(find_python())
                .arg(&py_path)
                .arg(&json_path)
                .output()
                .map_err(|e| format!("Python が起動できません: {e}"))?;
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("原因不明");
                return Err(if err.contains("No module named") {
                    format!("matplotlib がありません({last})。次で入ります:\n  {}",
                            pyrun::pip_hint("matplotlib"))
                } else {
                    format!("グラフが描けません: {last}")
                });
            }
            std::fs::read(&out).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(data) => {
                        let (w, h) = image_px(&data).unwrap_or((640, 400));
                        this.checkpoint();
                        this.sheet_mut().images_new.push(book::SheetImage {
                            at,
            dx_px: 0.0,
            dy_px: 0.0,
                            width_px: w as f32,
                            height_px: h as f32,
                            data,
                        });
                        this.dirty = true;
                        this.status = ui::tf!(
                            "placed_chart_goes_into",
                            at.a1()
                        )
                        .into();
                    }
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// ピボットテーブルの挿入(polars が裏方)。指図(PivotDef)を組んで
    /// 回すだけ — 置き直せるように指図はブックに控える(xl/joPivot.xml)。
    pub(crate) fn insert_pivot(
        &mut self,
        pend: PivotPend,
        value: String,
        agg: &'static str,
        cx: &mut Context<Self>,
    ) {
        // 組み替え(フィールドリスト)なら、総計などの性質と場所は据え置く
        let keep = pend.replace.and_then(|i| self.book.pivots.get(i).cloned());
        let def = book::PivotDef {
            sheet: self.book.sheets[self.active].name.clone(),
            src: (pend.a, pend.b),
            rows_sel: pend.rows_sel,
            cols_sel: pend.cols_sel,
            value,
            agg: agg.to_string(),
            totals: keep.as_ref().map(|d| d.totals).unwrap_or(true), // 既定で総計(本家と同じ)
            subtotals: keep.as_ref().map(|d| d.subtotals).unwrap_or(false),
            blank_rows: keep.as_ref().map(|d| d.blank_rows).unwrap_or(false),
            compact: keep.as_ref().map(|d| d.compact).unwrap_or(false),
            dest: keep.as_ref().map(|d| d.dest).unwrap_or(pend.a), // 仮 — 置くときに決める
            size: keep.as_ref().map(|d| d.size).unwrap_or((0, 0)),
            hide: keep.as_ref().map(|d| d.hide.clone()).unwrap_or_default(),
            // 組み替えでも図は据え置く(付けた図が消えない)
            chart_at: keep.as_ref().and_then(|d| d.chart_at),
            style: keep.as_ref().map(|d| d.style.clone()).unwrap_or_default(),
            vfilter: keep.as_ref().and_then(|d| d.vfilter.clone()),
            group_by: keep.as_ref().map(|d| d.group_by.clone()).unwrap_or_default(),
            show_as: keep.as_ref().map(|d| d.show_as.clone()).unwrap_or_default(),
            sort: keep.as_ref().map(|d| d.sort.clone()).unwrap_or_default(),
            name: keep.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| {
                // 新しい名前(ピボットテーブル1, 2, …)。空きの番号を探す
                let mut n = 1;
                loop {
                    let name = format!("ピボットテーブル{n}");
                    if !self.book.pivots.iter().any(|d| d.name == name) {
                        break name;
                    }
                    n += 1;
                }
            }),
        };
        self.spawn_pivot(def, pend.replace, cx);
    }

    /// いまのシートで、この位置に置いてあるピボットの指図の番号。
    pub(crate) fn pivot_at(&self, p: Pos) -> Option<usize> {
        let name = &self.book.sheets[self.active].name;
        self.book.pivots.iter().position(|d| {
            d.sheet == *name
                && d.size.0 > 0
                && p.row >= d.dest.row
                && p.row < d.dest.row + d.size.0
                && p.col >= d.dest.col
                && p.col < d.dest.col + d.size.1
        })
    }

    /// 集計の面をセルに書く。種別で見た目を付ける(h=見出し行の色、
    /// s=小計 t=総計は太字、t は上罫線も)。
    /// tot_col = 右端が総計の列(装いを効かせる)。本家のピボットの見た目
    /// (濃い見出し行・太字の総計)に寄せる — 出力そのものがピボットだと分かる
    pub(crate) fn place_pivot_grid(
        &mut self,
        si: usize,
        at: Pos,
        grid: &[Vec<String>],
        kinds: &[char],
        tot_col: bool,
        style: &str,
    ) {
        // 見た目の組(スタイルギャラリー)。(見出しの地, 見出しの字, 小計の地)
        let (head_bg, head_fg, sub_bg) = match style {
            "green" => ("548235", "FFFFFF", "E2EFDA"),
            "orange" => ("C55A11", "FFFFFF", "FBE5D6"),
            "grey" => ("595959", "FFFFFF", "EDEDED"),
            _ => ("4472C4", "FFFFFF", "D9E1F2"), // 既定 = 青
        };
        paste_values_text(&mut self.book.sheets[si], at, grid);
        let w = grid.iter().map(|r| r.len()).max().unwrap_or(1) as u32;
        for (i, k) in kinds.iter().enumerate() {
            let last = kinds.len() - 1;
            for c in 0..w {
                let p = Pos::new(at.row + i as u32, at.col + c);
                let mut cell = self.book.sheets[si].get(p).cloned().unwrap_or_default();
                match k {
                    'l' | 'h' => {
                        // 見出し行の色(スタイルの色)
                        cell.fmt.bold = true;
                        cell.fmt.fill = Some(head_bg.into());
                        cell.fmt.color = Some(head_fg.into());
                    }
                    's' => {
                        cell.fmt.bold = true;
                        cell.fmt.fill = Some(sub_bg.into());
                    }
                    't' => {
                        cell.fmt.bold = true;
                        cell.fmt.borders.top = book::Edge::THIN;
                    }
                    _ => {}
                }
                // 総計の列(右端)も太字+仕切り線
                if tot_col && c == w - 1 && *k != 'h' {
                    cell.fmt.bold = true;
                    cell.fmt.borders.left = book::Edge::THIN;
                }
                // 塊の外周に薄い線(印刷でも塊が分かる)
                if i == 0 { cell.fmt.borders.top = book::Edge::THIN; }
                if i == last { cell.fmt.borders.bottom = book::Edge::THIN; }
                if c == 0 { cell.fmt.borders.left = book::Edge::THIN; }
                if c == w - 1 { cell.fmt.borders.right = book::Edge::THIN; }
                self.book.sheets[si].set(p, cell);
            }
        }
    }

    /// 指図どおりに polars を回して置く。replace=None は挿入(右の空きを探す)、
    /// Some(i) は i 番の指図の更新(同じ場所に置き直す)。
    pub(crate) fn spawn_pivot(
        &mut self,
        mut def: book::PivotDef,
        replace: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(si) = self.book.sheets.iter().position(|s| s.name == def.sheet) else {
            self.status = ui::tf!("no_sheet_pivots_source", def.sheet).into();
            return;
        };
        let (a, b) = def.src;
        let sh = &self.book.sheets[si];
        let headers: Vec<String> = (a.col..=b.col)
            .map(|c| {
                let v = sh.get(Pos::new(a.row, c)).map(|x| x.value.display()).unwrap_or_default();
                if v.is_empty() { col_name(c) } else { v }
            })
            .collect();
        let data: Vec<Vec<String>> = (a.row + 1..=b.row)
            .map(|r| {
                (a.col..=b.col)
                    .map(|c| sh.get(Pos::new(r, c)).map(|x| x.value.display()).unwrap_or_default())
                    .collect()
            })
            .collect();
        // **集計は Rust の polars で、その場で回します**(2026-08-29 発注者
        // 「ピボットの処理は polars をつかって」)。前は Python を別プロセスで
        // 起こしていて、1万行で 145ms 待っていました。いまは 1ms です。
        // 待たないので、進み具合の札も要りません
        // **画面に出る札は Rust で訳してから渡します**(2026-08-26 の決め)。
        // 集計の側は鍵で処理し、字は渡された訳で書きます
        let mut spec = pivot::from_def(&def);
        // `subtotal` の訳は「{} 小計」の形です(`{}` が区切りの名前)
        spec.subtotal_label = ui::t!("subtotal").to_string();
        spec.grand_label = ui::t!("grand_totals").to_string();
        spec.agg_label = ui::tr_dyn(&def.agg).to_string();
        let task = std::future::ready(
            pivot::run(&headers, &data, &spec)
                .map(|g| (g.rows, g.kinds))
                .map_err(|e| e.to_string()),
        );
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((grid, kinds)) => {
                        let h = grid.len() as u32;
                        let w = grid.iter().map(|r| r.len()).max().unwrap_or(1) as u32;
                        let used = |this: &Self, p: Pos| {
                            this.book.sheets[si]
                                .get(p)
                                .map(|cell| {
                                    !cell.value.display().is_empty() || cell.formula.is_some()
                                })
                                .unwrap_or(false)
                        };
                        match replace {
                            None => {
                                // 右の空きを探す(埋まっていたらさらに右へ。黙って上書きしない)
                                let mut dc = b.col + 2;
                                let mut tries = 0;
                                let free = loop {
                                    let occupied = (0..h).any(|r| {
                                        (0..w).any(|c| used(this, Pos::new(a.row + r, dc + c)))
                                    });
                                    if !occupied {
                                        break true;
                                    }
                                    dc += w + 1;
                                    tries += 1;
                                    if tries > 50 {
                                        break false;
                                    }
                                };
                                if !free {
                                    this.status =
                                        ui::t!("no_free_space_right").into();
                                } else {
                                    this.checkpoint_book();
                                    def.dest = Pos::new(a.row, dc);
                                    def.size = (h, w);
                                    let at = def.dest;
                                    let tot_col = def.totals && !def.cols_sel.is_empty();
                                    let style = def.style.clone();
                                    this.place_pivot_grid(si, at, &grid, &kinds, tot_col, &style);
                                    recalc_book(&mut this.book, si);
                                    let (value, agg) = (def.value.clone(), def.agg.clone());
                                    this.book.pivots.push(def);
                                    this.dirty = true;
                                    let pi = this.book.pivots.len() - 1;
                                    this.pivot_chart_redraw(pi, cx);
                                    // カーソルを置いた集計へ移し、ピボットテーブルの
                                    // タブを開く(本家の showPivotTab と同じ)。
                                    // 文脈タブに気づかないままにしない
                                    this.anchor = None;
                                    this.cursor = at;
                                    if let Some(ti) = ribbon::calc_tabs()
                                        .iter()
                                        .position(|t| t.cmds.iter().any(|c| c.id == "pivot-layout"))
                                    {
                                        if this.tab != ti {
                                            this.prev_tab = this.tab;
                                            this.tab = ti;
                                        }
                                    }
                                    this.sync_input();
                                    let pname = this
                                        .book
                                        .pivots
                                        .last()
                                        .map(|d| d.name.clone())
                                        .unwrap_or_default();
                                    this.status = ui::tf!(
                                        "placed_values_now_pivottable",
                                        pname,
                                        value,
                                        agg,
                                        at.a1()
                                    )
                                    .into();
                                }
                            }
                            Some(pi) => {
                                let Some(old) = this.book.pivots.get(pi).cloned() else {
                                    return;
                                };
                                let dest = old.dest;
                                let in_old = |p: Pos| {
                                    p.row >= dest.row
                                        && p.row < dest.row + old.size.0
                                        && p.col >= dest.col
                                        && p.col < dest.col + old.size.1
                                };
                                let occupied = (0..h).any(|r| {
                                    (0..w).any(|c| {
                                        let p = Pos::new(dest.row + r, dest.col + c);
                                        !in_old(p) && used(this, p)
                                    })
                                });
                                if occupied {
                                    this.status =
                                        ui::t!("grown_area_blocked_clear").into();
                                } else {
                                    this.checkpoint_book();
                                    for r in 0..old.size.0 {
                                        for c in 0..old.size.1 {
                                            this.book.sheets[si]
                                                .cells
                                                .remove(&Pos::new(dest.row + r, dest.col + c));
                                        }
                                    }
                                    def.dest = dest;
                                    def.size = (h, w);
                                    // 装いは**新しい指図**(def)に合わせる — old だと
                                    // 総計を入切した直後の更新で右端の太字がずれる
                                    let tot_col = def.totals && !def.cols_sel.is_empty();
                                    let style = def.style.clone();
                                    this.place_pivot_grid(si, dest, &grid, &kinds, tot_col, &style);
                                    recalc_book(&mut this.book, si);
                                    this.book.pivots[pi] = def;
                                    this.dirty = true;
                                    // **図もここで描き直します。**ピボットが
                                    // 変わったのに図が古いままだと、同じ画面に
                                    // 食い違う2つの数字が並びます
                                    this.pivot_chart_redraw(pi, cx);
                                    this.sync_input();
                                    this.status = ui::tf!(
                                        "refreshed_pivot_values_now",
                                        dest.a1()
                                    )
                                    .into();
                                }
                            }
                        }
                    }
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Python in Calc(発注者提案 2026-08-04)。**コードは文書に入れない** —
    /// マクロと違い「開く=実行」の経路が無い。いまの表を一時 xlsx に写し、
    /// office_sheet(pysheet)で b(ブック)と s(いまのシート)を束縛して
    /// 利用者のコードを回し、保存されたものを読み戻して**1手として**適用する。
    pub(crate) fn run_python(&mut self, user_code: String, cx: &mut Context<Self>) {
        // 自分で打った/選んだコード: サンドボックスはかけるが網は許す(自分の道具が
        // Web から取り込むのは普通の仕事。守るのは機械のファイルの方)
        self.run_python_inner(user_code, false, true, cx);
    }

    /// sandbox=true は**必ず**bubblewrap のサンドボックスの中で回す(ブックに載っていた
    /// コード = 他人のファイル由来かもしれないもの)。サンドボックス: ネット遮断・
    /// 実ファイルは読み取り専用・ホームは不可視・書けるのは交換用の一時領域だけ。
    /// サンドボックスが無い機械では載せたコードは**実行しない**(そう言う)。
    /// 自分で打った/選んだコードも、サンドボックスがあればサンドボックスで回す(深層防御)。
    pub(crate) fn run_python_inner(
        &mut self,
        user_code: String,
        sandbox: bool,
        allow_net: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.commit() {
            return;
        }
        let dir = cage_work_dir("jo-py");
        let _ = std::fs::create_dir_all(&dir);
        let in_x = dir.join("in.xlsx");
        let out_x = dir.join("out.xlsx");
        // 実行は複製の上(失敗しても表は無傷)。原本の部品も持ち越して写す
        let original: Option<std::io::Cursor<Vec<u8>>> = self
            .path
            .as_ref()
            .and_then(|old| std::fs::read(old).ok())
            .map(std::io::Cursor::new);
        let w = std::fs::File::create(&in_x)
            .map_err(|e| e.to_string())
            .and_then(|f| {
                sheet::xlsx::write_with(&self.book, original, std::io::BufWriter::new(f))
            });
        if let Err(e) = w {
            self.status = ui::tf!("cant_hand_python", e).into();
            return;
        }
        // officework は実行ファイルの隣か、pip で入れた物(HIKITSUGI の配り方)。
        // **エンジンは officework.sheet**(2026-08-09 に office_sheet から
        // 移した — docx / pptx が来ても同じ名前空間に入るように)
        let so_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        let so_dir2 = so_dir.clone();
        let script = format!(
            concat!(
                "import sys\n",
                "sys.path.insert(0, {so_dir:?})\n",
                "from officework import sheet as office_sheet\n",
                "b = office_sheet.Book.open({in_x:?})\n",
                "s = b[{active}]\n",
                "# ---- 利用者のコード ----\n",
                "{code}\n",
                "# ----\n",
                "b.save({out_x:?})\n"
            ),
            so_dir = so_dir.to_string_lossy(),
            in_x = in_x.to_string_lossy(),
            active = self.active,
            out_x = out_x.to_string_lossy(),
            code = user_code
        );
        self.status = ui::t!("running_python").into();
        let task = cx.background_executor().spawn(async move {
            let py_path = dir.join("run.py");
            std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
            let py = find_python();
            // サンドボックスはあれば必ず使う(深層防御)。他所から来たかもしれないコード
            // (sandbox=true)は、サンドボックスが組めなければ実行しない
            let venv = std::fs::canonicalize(".venv").unwrap_or_default();
            // 見せる場所: venv・実行ファイルの隣・editable の実体(.pth の先)。
            // pip install -e の形は実体が venv の外にあり、見せないと
            // サンドボックスの中で officework が読めない
            let mut binds = vec![venv.clone(), so_dir2];
            binds.extend(pyrun::editable_paths(&venv));
            let mut cmd = match caged_python(&py, &dir, &binds, allow_net) {
                Some(c) => c,
                // **案内は OS で変える。** 前は3つの OS すべてに
                // 「apt install bubblewrap」と出していましたが、macOS と
                // Windows にこの道具はありません。直しようのない指示を
                // 出すと、利用者は自分の操作を疑って時間を使います
                None if sandbox => {
                    return Err(if cfg!(target_os = "linux") {
                        ui::t!(
                            "cant_build_sandbox_code"
                        )
                        .to_string()
                    } else {
                        ui::t!(
                            "os_no_sandbox_code"
                        )
                        .to_string()
                    });
                }
                None => std::process::Command::new(&py),
            };
            // 時間制限つき(60秒)— サンドボックスの中の無限ループで手が塞がらない
            let (ok, out, err) = run_with_timeout(cmd.arg(&py_path), 60)?;
            let out = out.trim().to_string();
            if !ok {
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("原因不明")
                    .to_string();
                return Err(if err.contains("No module named 'officework'")
                    || err.contains("エンジン(_sheet)が読めません")
                {
                    ui::t!("officework_engine_missing_pip").to_string()
                } else {
                    last
                });
            }
            std::fs::read(&out_x)
                .map_err(|e| format!("結果が読めません: {e}"))
                .map(|bytes| (bytes, out))
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok((bytes, out)) => {
                        match sheet::xlsx::read(std::io::Cursor::new(bytes)) {
                            Ok((mut book, rep)) => {
                                book::calc::recalc_all(&mut book);
                                this.checkpoint_book();
                                this.book = book;
                                if this.active >= this.book.sheets.len() {
                                    this.active = 0;
                                }
                                // ファイルの固定枠を画面へ(sheet_ui も作り直す)
                                this.freeze_from_book();
                                this.dirty = true;
                                this.sync_input();
                                this.notes = rep
                                    .unsupported
                                    .iter()
                                    .map(|(n, c)| SharedString::from(format!("{n} × {c}")))
                                    .collect();
                                this.status = if out.is_empty() {
                                    ui::t!("python_finished_one_ctrl").into()
                                } else {
                                    let last =
                                        out.lines().last().unwrap_or_default().to_string();
                                    format!(
                                        "Python: {last}(出力{}行。変更は Ctrl+Z で戻せます)",
                                        out.lines().count()
                                    )
                                    .into()
                                };
                            }
                            Err(e) => {
                                this.status = ui::tf!("cant_read_result", e).into();
                            }
                        }
                    }
                    Err(e) => this.status = format!("Python: {e}").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 裏で 200ms ごとに見て、UDF の引数が変わっていたら計算し直す。
    /// 指紋(sheet の py_stamp)が動いたときだけ働くので、UDF の無いブックでは
    /// 何も起きない。二重には走らせない(udf_busy)。
    pub(crate) fn udf_tick(&mut self, cx: &mut Context<Self>) {
        if self.udf_busy || self.prompt.is_some() {
            return;
        }
        // **計算し直す物があるかだけ見る。** ここで指紋を控えてはいけない —
        // 走っている最中に増えたセルまで「計算済み」になり、二度と走らない
        // (2026-08-14。表の3行目が #PY? のまま残った)
        if !self.udf_dirty() {
            return;
        }
        self.run_udfs(true, cx);
    }

    /// 引数の変わった UDF のセルがあるか(**セルごとの指紋**で見る)
    pub(crate) fn udf_dirty(&self) -> bool {
        self.book.sheets.iter().enumerate().any(|(i, sh)| {
            sh.cells.iter().any(|(p, c)| {
                c.formula.as_ref().is_some_and(|f| book::calc::is_py_formula(f))
                    && book::calc::py_cell_stamp(sh, *p)
                        != self.udf_stamp.get(&(i, *p)).copied()
            })
        })
    }

    /// UDF のセルを全部計算する(@計算 の手回し)。
    pub(crate) fn run_py_calc(&mut self, cx: &mut Context<Self>) {
        if !self.commit() {
            return;
        }
        self.run_udfs(false, cx);
    }

    /// UDF のセルを別スレッドでまとめて計算し、答えが揃ってから1手で書き戻す。
    /// 関数の定義は **plugins の .py**(ブックはコードを運ばない)。
    /// auto=true は自動の計算 — undo の節目を作らず、書きかけの印も動かさない。
    /// **サンドボックスは着せない**: 回すのは自分で plugins に置いたコードだけで、
    /// ブックから旅して来たコードではない(2026-08-09 発注者確定)。
    fn run_udfs(&mut self, auto: bool, cx: &mut Context<Self>) {
        let mut per_sheet: Vec<CallsPerSheet> =
            Vec::new();
        // 投げたセルの控え(答えが返った時、この分だけ指紋を控える)
        let mut sent: Vec<(usize, Vec<Pos>)> = Vec::new();
        for (i, sh) in self.book.sheets.iter().enumerate() {
            let mut calls = Vec::new();
            let mut cells = Vec::new();
            for (p, c) in &sh.cells {
                let Some(f) = &c.formula else { continue };
                if !book::calc::is_py_formula(f) {
                    continue;
                }
                // **変わっていないセルは投げない**(発注者 2026-08-14
                // 「UDF の呼び出しは重い」)。100 行あるとき、1つ直すたびに
                // 100 回ぶんを投げ直していた。手回し(@計算)のときは全部
                let changed = book::calc::py_cell_stamp(sh, *p)
                    != self.udf_stamp.get(&(i, *p)).copied();
                if !auto || changed {
                    if let Some((name, args)) = book::calc::eval_py_call(sh, f) {
                        calls.push((p.a1(), name, args));
                        cells.push(*p);
                    }
                }
            }
            if !calls.is_empty() {
                per_sheet.push((i, calls));
                sent.push((i, cells));
            }
        }
        if std::env::var_os("JO_UDF_LOG").is_some() {
            for (i, cs) in &per_sheet {
                eprintln!(
                    "[udf] シート{i}: {} 件 → {:?}",
                    cs.len(),
                    cs.iter().map(|(c, n, a)| (c.as_str(), n.as_str(), a.len())).collect::<Vec<_>>()
                );
            }
        }
        if per_sheet.is_empty() {
            if !auto {
                self.status = ui::t!("no_cell_calls_function").into();
            }
            return;
        }
        // 呼ばれている名前を plugins の .py に結ぶ。足りなければ名指しで言う
        let names: Vec<String> = {
            let mut v: Vec<String> = per_sheet
                .iter()
                .flat_map(|(_, cs)| cs.iter().map(|(_, n, _)| n.clone()))
                .collect();
            v.sort();
            v.dedup();
            v
        };
        let mut resolved: std::collections::HashMap<String, (String, String)> = Default::default();
        let mut missing: Vec<String> = Vec::new();
        for n in &names {
            match resolve_udf(n) {
                Ok(mf) => {
                    resolved.insert(n.clone(), mf);
                }
                Err(e) => missing.push(e),
            }
        }
        if !missing.is_empty() {
            // **式から呼ぶ関数の置き場は funcs**(2026-08-16)
            self.status = ui::tf!("put_py", missing.join(" / "), pyrun::funcs_dir().display()).into();
            return;
        }
        // 使うモジュールだけ読む(呼ばれていない .py は動かさない)
        let mut mods: Vec<(String, String)> = Vec::new();
        for (m, _) in resolved.values() {
            if mods.iter().any(|(n, _)| n == m) {
                continue;
            }
            match std::fs::read_to_string(pyrun::funcs_dir().join(format!("{m}.py"))) {
                Ok(src) => mods.push((m.clone(), src)),
                Err(e) => {
                    self.status = ui::tf!("cant_read_py", m, e).into();
                    return;
                }
            }
        }
        // (セル, モジュール, 関数, 引数)へ組み替える
        let per_sheet: Vec<FormulaCallsPerSheet> =
            per_sheet
                .into_iter()
                .map(|(i, calls)| {
                    let calls = calls
                        .into_iter()
                        .map(|(cell, name, args)| {
                            let (m, f) = resolved[name.as_str()].clone();
                            (cell, m, f, args)
                        })
                        .collect();
                    (i, calls)
                })
                .collect();
        let dir = cage_work_dir("jo-udf");
        let _ = std::fs::create_dir_all(&dir);
        let mut scripts = Vec::new();
        for (i, calls) in &per_sheet {
            let out = dir.join(format!("out{i}.txt"));
            scripts.push((
                *i,
                dir.join(format!("udf{i}.py")),
                out.clone(),
                build_udf_script(&mods, calls, &out),
            ));
        }
        if !auto {
            self.status = ui::t!("calculating_function").into();
        }
        self.udf_busy = true;
        let task = cx.background_executor().spawn(async move {
            let py = find_python();
            let funcs = pyrun::funcs_dir();
            let mut results = Vec::new();
            for (i, py_path, out_path, script) in scripts {
                std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
                // 関数もサンドボックスの中で走らせる。呼び出しはシートごとに
                // 1回にまとめてあるので、サンドボックスの起動(数十ミリ秒)は1セルごとには
                // 掛からない。funcs の置き場は読み取り専用で見せる。
                // 時間制限つき(30秒)。関数は値の計算だけ — それより長いのは異常
                let mut c = caged_or_plain(&py, &dir, &[funcs.clone()]);
                let (ok, _, err) = run_with_timeout(c.arg(&py_path), 30)?;
                if !ok {
                    let last = err
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("原因不明");
                    return Err(format!("PY の計算に失敗: {last}"));
                }
                let raw = std::fs::read_to_string(&out_path).unwrap_or_default();
                results.push((i, raw));
            }
            Ok(results)
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                this.udf_busy = false;
                match r {
                    Ok(outs) => {
                        // 自動の計算は undo の節目を作らない(利用者の1手ではない)
                        if !auto {
                            this.checkpoint_book();
                        }
                        let (mut total, mut conflicts) = (0usize, 0usize);
                        for (i, raw) in outs {
                            let results = parse_udf_output(&raw);
                            let prev: std::collections::HashMap<Pos, (u32, u32)> = this
                                .py_spills
                                .iter()
                                .filter(|((si, _), _)| *si == i)
                                .map(|((_, p), d)| (*p, *d))
                                .collect();
                            let (spills, n, c) =
                                apply_py_results(&mut this.book.sheets[i], &results, &prev);
                            this.py_spills.retain(|(si, _), _| *si != i);
                            for (p, d) in spills {
                                this.py_spills.insert((i, p), d);
                            }
                            recalc_book(&mut this.book, i);
                            total += n;
                            conflicts += c;
                        }
                        // **投げたセルの指紋だけ**を控える(同じ引数で回り
                        // 続けないため)。**全部を控え直してはいけない** —
                        // 走っている間に増えたセルまで「計算済み」になり、
                        // 二度と走らない(2026-08-14。表の3行目が #PY? の
                        // まま残った本当の原因)。投げていない物は指紋を
                        // 持たないまま残り、次の見回りで拾われる
                        for (i, cells) in &sent {
                            let sh = &this.book.sheets[*i];
                            for p in cells {
                                match book::calc::py_cell_stamp(sh, *p) {
                                    Some(st) => {
                                        this.udf_stamp.insert((*i, *p), st);
                                    }
                                    // 式でなくなった(消された)— 控えも消す
                                    None => {
                                        this.udf_stamp.remove(&(*i, *p));
                                    }
                                }
                            }
                        }
                        this.sync_input();
                        if conflicts > 0 {
                            this.status = ui::tf!(
                                "functions_computed_cells_gave",
                                total,
                                conflicts
                            )
                            .into();
                        } else if !auto {
                            this.dirty = true;
                            this.status =
                                ui::tf!("functions_computed_cells_ctrl", total).into();
                        }
                    }
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// plugins の手続きを走らせる。**動いている calc をそのまま操る** —
    /// 一時ファイルの複製ではなく、子のプロセスが officework のソケット越しに
    /// このブックを書く(記事の「ファイルではなく Excel そのものを操作する」)。
    /// 何回書いても Ctrl+Z 一回で戻る(頭で節目を1つ置き、その間 rpc は置かない)。
    /// **サンドボックスの中で走らせる**(手引き macro-manual「サンドボックス」)。
    /// 開いているブックへの書き込みはソケット越しなので、ソケットの置き場だけ
    /// サンドボックスの中に見せる。bwrap が無い機械では今までどおり普通に走らせる。
    pub(crate) fn run_plugin(&mut self, module: &str, func: Option<&str>, cx: &mut Context<Self>) {
        self.run_plugin_in(plugins_dir(), module, func, cx);
    }

    /// 置き場を選んで手続きを走らせる。**置き場は2つ** — plugins(人が一覧から
    /// 選ぶ)と ribbon(利用者がリボンに足したボタン)。走り方は同じで、
    /// 違うのは呼ばれ方だけ
    pub(crate) fn run_plugin_in(
        &mut self,
        dir_py: std::path::PathBuf,
        module: &str,
        func: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if !self.commit() {
            return;
        }
        let script = format!(
            concat!(
                "import sys\n",
                "sys.path.insert(0, {dir:?})\n",
                "import importlib\n",
                "m = importlib.import_module({module:?})\n",
                "{call}"
            ),
            dir = dir_py.to_string_lossy(),
            module = module,
            call = match func {
                Some(f) => format!("getattr(m, {f:?})()\n"),
                None => String::new(), // 取り込むだけ(昔ながらの「上から下まで走る .py」)
            }
        );
        let dir = cage_work_dir("jo-plugin");
        let _ = std::fs::create_dir_all(&dir);
        let name = match func {
            Some(f) => format!("{module}.{f}"),
            None => module.to_string(),
        };
        self.checkpoint_book();
        self.rpc_batch = true;
        let caged = pyrun::cage_works();
        self.status = if caged {
            ui::tf!("running_macro_python_sandbox", name.clone()).into()
        } else {
            ui::tf!("running", name.clone()).into()
        };
        let task = cx.background_executor().spawn(async move {
            let py_path = dir.join("plugin.py");
            std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
            let py = find_python();
            // 置き場(plugins / ribbon)は読み取り専用で見せる。サンドボックスは
            // ホームを隠すので、見せないと import で落ちる
            let mut c = caged_or_plain(&py, &dir, &[dir_py.clone()]);
            // 時間制限つき(60秒)。返りは (終わったか, 出力, 誤り)
            let (ok, out, err) = run_with_timeout(c.arg(&py_path), 60)?;
            if !ok {
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("原因不明")
                    .to_string();
                return Err(last);
            }
            Ok(out.trim().to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                this.rpc_batch = false;
                this.sync_input();
                this.status = match r {
                    Ok(out) if out.is_empty() => {
                        ui::tf!("ran_ctrl_z_undoes", name).into()
                    }
                    Ok(out) => format!(
                        "{name}: {}(出力{}行。Ctrl+Z で1手で戻せます)",
                        out.lines().last().unwrap_or_default(),
                        out.lines().count()
                    )
                    .into(),
                    Err(e) => format!("{name}: {e}").into(),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// 古いブックに載っていたコードを .py に取り出す。**実行はしない** —
    /// 中身を確かめてから plugins へ置くのは人の手(それが取り込みの門)
    pub(crate) fn export_python_dialog(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(code) = self
            .book
            .scripts
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| c.clone())
        else {
            self.status = ui::tf!("not_exist_list_lists", name).into();
            return;
        };
        let fname = format!("{name}.py");
        let ask = cx.background_executor().spawn(async move {
            rfd::FileDialog::new()
                .add_filter("python", &["py"])
                .set_file_name(&fname)
                .save_file()
        });
        cx.spawn(async move |this, cx| {
            let r = ask.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(p) = r {
                    this.status = match std::fs::write(&p, &code) {
                        Ok(_) => ui::tf!(
                            "took_out_check_contents",
                            p.display().to_string()
                        )
                        .into(),
                        Err(e) => ui::tf!("cant_write", e.to_string()).into(),
                    };
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// .py ファイルを選んで回す(コードは利用者のファイルにある —
    /// CSV/TSV を選んで、ウィザードのパネル(文字コード・区切り・置き場所・
    /// プレビュー)を開く。読み直しは Python(CP932 も読める)。
    pub(crate) fn import_text_dialog(&mut self, cx: &mut Context<Self>) {
        let ask = cx
            .background_executor()
            .spawn(async {
                rfd::FileDialog::new()
                    .add_filter("テキストのデータ・PDF", &["csv", "tsv", "txt", "pdf"])
                    .pick_file()
            });
        cx.spawn(async move |this, cx| {
            let Some(p) = ask.await else { return };
            let _ = this.update(cx, |this, cx| {
                this.import_pend = Some(ImportPend {
                    path: p,
                    enc: 0,
                    delim: 0,
                    custom: String::new(),
                    dest: this.cursor,
                    grid: Vec::new(),
                    used: (String::new(), String::new()),
                    pdf: Vec::new(),
                    pdf_at: 0,
                });
                this.import_reparse(cx);
            });
        })
        .detach();
    }

    /// いまの設定(文字コード・区切り)でファイルを読み直し、パネルを出し直す。
    pub(crate) fn import_reparse(&mut self, cx: &mut Context<Self>) {
        let Some(pend) = &self.import_pend else { return };
        // PDF は別の道。文字コードも区切りも関わりません
        if pend.path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            self.pdf_reparse(cx);
            return;
        }
        let (path, enc, delim) = (
            pend.path.clone(),
            import_encs()[pend.enc].2.to_string(),
            match import_delims()[pend.delim].2 {
                "その他" => {
                    if pend.custom.is_empty() { ",".into() } else { pend.custom.clone() }
                }
                d => d.to_string(),
            },
        );
        let job = cx.background_executor().spawn(async move {
            let dir = workdir("csv");
            let _ = std::fs::create_dir_all(&dir);
            // csv.py という名前は標準ライブラリの csv を隠してしまう(踏んだ)
            let py_path = dir.join("jo_csv.py");
            if std::fs::write(&py_path, CSV_PY).is_err() {
                return Err(ui::t!("cant_write_temporary_file").to_string());
            }
            let o = std::process::Command::new(find_python())
                .arg(&py_path)
                .arg(&path)
                .arg(&enc)
                .arg(&delim)
                .output();
            match o {
                Ok(o) if o.status.success() => {
                    Ok(String::from_utf8_lossy(&o.stdout).to_string())
                }
                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                Err(e) => Err(format!("Python が起動できません: {e}")),
            }
        });
        cx.spawn(async move |this, cx| {
            let r = job.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(data) => {
                        let mut rows = data.split('\u{1e}');
                        // 1行目は下ごしらえの報告(\x01 文字コード \x1f 区切り)
                        let meta = rows.next().unwrap_or_default();
                        let used = meta
                            .strip_prefix('\u{01}')
                            .and_then(|m| m.split_once('\u{1f}'))
                            .map(|(e, d)| (e.to_string(), d.to_string()))
                            .unwrap_or_default();
                        let grid: Vec<Vec<String>> = rows
                            .map(|row| row.split('\u{1f}').map(|f| f.to_string()).collect())
                            .collect();
                        if let Some(pend) = &mut this.import_pend {
                            pend.grid = grid;
                            pend.used = used;
                        }
                        this.import_pick();
                    }
                    Err(e) => {
                        this.status = ui::tf!("not_read", e).into();
                        this.import_pick(); // パネルは残す(設定を替えて再挑戦できる)
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 取り込みウィザードのパネルを(いまの下ごしらえから)組む。
    /// 予測の答えを新しいシートに置く。
    ///
    /// 形は Excel と同じで、**実績の行と予測の行を分けます**。境目の1点だけは
    /// 両方に入れます — 分けたままだと折れ線が切れて見えるためです。
    fn forecast_place(
        &mut self,
        raw: &str,
        labels: Vec<String>,
        values: Vec<f64>,
        heading: String,
        cx: &mut Context<Self>,
    ) {
        let numbers = |k: &str| -> Vec<f64> {
            raw.split_once(&format!("\"{k}\":["))
                .and_then(|(_, r)| r.split_once(']'))
                .map(|(body, _)| body.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                .unwrap_or_default()
        };
        let one_of = |k: &str| -> f64 {
            raw.split_once(&format!("\"{k}\":"))
                .and_then(|(_, r)| {
                    let t: String =
                        r.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e').collect();
                    t.parse().ok()
                })
                .unwrap_or(0.0)
        };
        let (fc, lo, up) = (numbers("forecast"), numbers("lower"), numbers("upper"));
        if fc.is_empty() {
            self.status = ui::t!("forecast_result_cannot_read").into();
            return;
        }
        let season = one_of("season") as u32;
        let sigma = one_of("sigma");
        self.checkpoint();
        let name = crate::util::unique_sheet_name_for(&self.book, ui::t!("forecast"));
        let mut sh = book::Sheet::new(&name);
        let actual = if heading.is_empty() { ui::t!("actual").to_string() } else { heading };
        for (c, t) in [
            (0u32, ui::t!("period").to_string()),
            (1, actual),
            (2, ui::t!("forecast").to_string()),
            (3, ui::t!("lower").to_string()),
            (4, ui::t!("upper").to_string()),
        ] {
            sh.set(Pos::new(0, c), book::Cell::input(&t));
        }
        let n = values.len();
        for (i, v) in values.iter().enumerate() {
            sh.set(Pos::new(i as u32 + 1, 0), book::Cell::input(&labels[i]));
            sh.set(Pos::new(i as u32 + 1, 1), book::Cell::input(&v.to_string()));
        }
        // 境目。**最後の実績を予測の列にも置く**(線を繋ぐため)
        sh.set(Pos::new(n as u32, 2), book::Cell::input(&values[n - 1].to_string()));
        for (j, v) in fc.iter().enumerate() {
            let r = (n + j + 1) as u32;
            sh.set(Pos::new(r, 0), book::Cell::input(&ui::tf!("forecast_2", j + 1)));
            sh.set(Pos::new(r, 2), book::Cell::input(&format!("{v:.2}")));
            sh.set(Pos::new(r, 3), book::Cell::input(&format!("{:.2}", lo[j])));
            sh.set(Pos::new(r, 4), book::Cell::input(&format!("{:.2}", up[j])));
        }
        // **断りはシートに残します。**状態行はこの後グラフの報せで流れるので、
        // そこだけに書くと、区間の意味が誰にも伝わりません
        let season = if season > 1 {
            ui::tf!("season_periods_found_automatically", season).to_string()
        } else {
            ui::t!("no_season_found").to_string()
        };
        let note_div = ui::tf!(
            "forecast_periods_ahead_interval",
            fc.len(),
            season.clone(),
            sigma
        )
        .to_string();
        sh.set(Pos::new((n + fc.len() + 2) as u32, 0), book::Cell::input(&note_div));
        self.book.sheets.push(sh);
        let si = self.book.sheets.len() - 1;
        self.switch_sheet(si);
        self.dirty = true;
        self.anchor = Some(Pos::new((n + fc.len()) as u32, 4));
        self.cursor = Pos::new(0, 0);
        self.insert_chart_kind(Pos::new(0, 0), Pos::new((n + fc.len()) as u32, 4), "line", cx);
        self.anchor = None;
        self.status = note_div.into();
    }

    /// **ピボットに連動する図を描き直す**(ピボットグラフ。2026-08-22)。
    ///
    /// 指図に置き場(`chart_at`)が入っているときだけ働きます。ピボットを
    /// 置き直すたびに呼ばれるので、**図がピボットに遅れません** — 遅れると、
    /// 同じ画面に食い違う2つの数字が並びます。
    pub(crate) fn pivot_chart_redraw(&mut self, pi: usize, cx: &mut Context<Self>) {
        let Some(d) = self.book.pivots.get(pi).cloned() else { return };
        let Some(at) = d.chart_at else { return };
        let Some(si) = self.book.sheets.iter().position(|s| s.name == d.sheet) else { return };
        if self.active != si {
            return; // 別のシートを見ている間は描かない(絵は開いたときに追いつく)
        }
        // 古い図を外す。**同じ場所に重ねない**
        self.book.sheets[si].images_new.retain(|im| im.at != at);
        // **総計は図に入れません。**入れると、総計の棒だけが飛び抜けて、
        // 他の棒が潰れて読めなくなります(実機で見た)
        let grand_row = u32::from(d.totals);
        let grand_col = u32::from(d.totals && !d.cols_sel.is_empty());
        let line = d.size.0.saturating_sub(grand_row);
        let row_box = d.size.1.saturating_sub(grand_col);
        if line < 2 || row_box < 2 {
            return;
        }
        let a = d.dest;
        let b = Pos::new(d.dest.row + line - 1, d.dest.col + row_box - 1);
        self.chart_dest = Some(at);
        self.insert_chart_kind(a, b, "bar", cx);
    }

    /// **予測シート**(2026-08-22。台帳の [大])。
    ///
    /// 選んだ範囲の**いちばん右の数の列**を実績とし、その左の列をラベルに
    /// します。指数平滑で先を出し、新しいシートに 実績・予測・下限・上限 を
    /// 並べて、折れ線のグラフを添えます。
    ///
    /// **区間は見込みであって約束ではありません。**そう状態行で言います。
    pub(crate) fn forecast_run(&mut self, h: usize, cx: &mut Context<Self>) {
        let (a, b) = if self.anchor.is_some() {
            self.sel_rect()
        } else {
            let (rows, cols) = self.sheet().extent();
            if rows < 5 || cols == 0 {
                self.status =
                    ui::t!("forecasting_needs_least_4").into();
                return;
            }
            (Pos::new(0, 0), Pos::new(rows - 1, cols - 1))
        };
        // 数の列を右から探す。ラベルはその左の列(無ければ番号)
        let sh = self.sheet();
        let num_cols = (a.col..=b.col).rev().find(|&c| {
            (a.row + 1..=b.row)
                .filter(|&r| matches!(sh.get(Pos::new(r, c)).map(|x| &x.value), Some(book::Value::Number(_))))
                .count()
                >= 4
        });
        let Some(vc) = num_cols else {
            self.status = ui::t!("no_column_numbers_found").into();
            return;
        };
        let lc = (vc > a.col).then(|| vc - 1);
        let mut labels: Vec<String> = Vec::new();
        let mut values: Vec<f64> = Vec::new();
        for r in a.row + 1..=b.row {
            let Some(book::Value::Number(v)) = sh.get(Pos::new(r, vc)).map(|x| x.value.clone())
            else {
                continue;
            };
            labels.push(match lc {
                Some(c) => sh.get(Pos::new(r, c)).map(|x| x.value.display()).unwrap_or_default(),
                None => (values.len() + 1).to_string(),
            });
            values.push(v);
        }
        if values.len() < 4 {
            self.status = ui::t!("forecasting_needs_least_4_numbers").into();
            return;
        }
        let heading = sh.get(Pos::new(a.row, vc)).map(|x| x.value.display()).unwrap_or_default();
        let json = format!(
            "{{\"values\":[{}],\"horizon\":{},\"conf\":0.95,\"season\":0}}",
            values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
            h
        );
        self.status = ui::t!("forecasting").into();
        let task = cx.background_executor().spawn(async move {
            let dir = workdir("forecast");
            let _ = std::fs::create_dir_all(&dir);
            let jp = dir.join("spec.json");
            let pp = dir.join("jo_forecast.py");
            std::fs::write(&jp, json).map_err(|e| e.to_string())?;
            std::fs::write(&pp, pyrun::FORECAST_PY).map_err(|e| e.to_string())?;
            let o = std::process::Command::new(find_python())
                .arg(&pp)
                .arg(&jp)
                .output()
                .map_err(|e| ui::tf!("cant_start_python", e).to_string())?;
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("原因不明");
                return Err(if err.contains("No module named") {
                    ui::tf!("tools_forecast_needs_missing",
                            pyrun::pip_hint("numpy scipy")).to_string()
                } else {
                    ui::tf!("cannot_forecast", last).to_string()
                });
            }
            Ok(String::from_utf8_lossy(&o.stdout).to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(raw) => this.forecast_place(&raw, labels, values, heading, cx),
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// **PDF から表を取り出す**(2026-08-21 の D群)。
    ///
    /// PDF に「表」という構造はありません。字と、その座標と、線だけです。
    /// 罫線があればそれで切り、無ければ文字の位置から推し量ります。
    /// **どちらで取ったかは画面に出します** — 推し量って取った表を、
    /// 正確に取れた表と同じ顔で出すと、ずれたまま気づけません。
    pub(crate) fn pdf_reparse(&mut self, cx: &mut Context<Self>) {
        let Some(pend) = &self.import_pend else { return };
        let path = pend.path.clone();
        self.status = ui::t!("looking_tables_pdf").into();
        let job = cx.background_executor().spawn(async move {
            let dir = workdir("pdf");
            let _ = std::fs::create_dir_all(&dir);
            let py_path = dir.join("jo_pdf.py");
            if std::fs::write(&py_path, pyrun::PDF_TABLE_PY).is_err() {
                return Err(ui::t!("cant_write_temporary_file").to_string());
            }
            let o = std::process::Command::new(find_python())
                .arg(&py_path)
                .arg(&path)
                .output();
            match o {
                Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).to_string()),
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    let last =
                        err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("原因不明");
                    Err(if err.contains("No module named") {
                        ui::tf!(
                            "tool_reads_pdfs_pdfplumber",
                            pyrun::pip_hint("pdfplumber")
                        )
                        .to_string()
                    } else {
                        ui::tf!("cannot_read_pdf", last).to_string()
                    })
                }
                Err(e) => Err(ui::tf!("cant_start_python", e).to_string()),
            }
        });
        cx.spawn(async move |this, cx| {
            let r = job.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(raw) => {
                        let tables = parse_pdf_tables(&raw);
                        if tables.is_empty() {
                            this.import_pend = None;
                            this.status = ui::t!(
                                "no_table_found_pdf"
                            )
                            .into();
                        } else if let Some(p) = &mut this.import_pend {
                            p.pdf_at = 0;
                            p.grid = tables[0].2.clone();
                            p.pdf = tables;
                            this.import_pick();
                        }
                    }
                    Err(e) => {
                        this.import_pend = None;
                        this.status = e.into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn import_pick(&mut self) {
        let Some(pend) = &self.import_pend else { return };
        let mut items: Vec<String> = Vec::new();
        let (encs, delims) = (import_encs(), import_delims());
        let enc_label = if pend.enc == 0 && !pend.used.0.is_empty() {
            format!("{}({})", encs[0].1, pend.used.0)
        } else {
            encs[pend.enc].1.to_string()
        };
        // 区切りの名も画面の字 — 訳す。読めない字はそのまま括って見せる
        let delim_name = |d: &str| match d {
            "," => ui::t!("comma").to_string(),
            "\t" => ui::t!("tab").to_string(),
            ";" => ui::t!("semicolon").to_string(),
            ":" => ui::t!("colon").to_string(),
            " " => ui::t!("space").to_string(),
            other => format!("「{other}」"),
        };
        let delim_label = if pend.delim == 0 && !pend.used.1.is_empty() {
            format!("{}({})", delims[0].1, delim_name(&pend.used.1))
        } else if delims[pend.delim].2 == "other_2" && !pend.custom.is_empty() {
            format!("{}{}", delims[pend.delim].1, delim_name(&pend.custom))
        } else {
            delims[pend.delim].1.to_string()
        };
        if pend.pdf.is_empty() {
            items.push(format!("{}: {}", ui::t!("encoding"), enc_label));
            items.push(format!("{}: {}", ui::t!("delimiter"), delim_label));
        } else {
            // PDF は文字コードも区切りも関わりません。代わりに
            // **どの表か**と、**どうやって取ったか**を出します
            let (page, how, _) = &pend.pdf[pend.pdf_at.min(pend.pdf.len() - 1)];
            items.push(format!(
                "{}: {}",
                ui::t!("table_2"),
                ui::tf!("page_3", pend.pdf_at + 1, pend.pdf.len(), page)
            ));
            // **台本は鍵だけを返します。**画面の字はここで訳します —
            // 台本が日本語を返すと、13言語で日本語が出ます
            let how_label = if how == "lines" {
                ui::t!("ruling_lines")
            } else {
                ui::t!("text_positions_guess")
            };
            items.push(format!("{}: {}", ui::t!("how_found"), how_label));
            if how == "text" {
                items.push(ui::t!("note_there_no_ruling")
                    .to_string());
            }
        }
        items.push(format!("{}: {}", ui::t!("destination"), pend.dest.a1()));
        // プレビュー(先頭3行。長ければ詰める)
        for (i, row) in pend.grid.iter().take(3).enumerate() {
            let mut line = row.join(" | ");
            if line.chars().count() > 42 {
                line = line.chars().take(42).collect::<String>() + "…";
            }
            items.push(format!("{} {}: {}", "·", i + 1, line));
        }
        items.push(format!(
            "→ {}",
            ui::tf!("import_rows", pend.grid.len())
        ));
        let at = self.pop_anchor();
        let name = pend
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.pick_note =
            Some(ui::tf!("text_import_click_line", name).into());
        self.pick_kind = "csv-import-pick";
        // この一覧は**訳した字がそのまま鍵**(受け口も ui::t! で頭を作って
        // 突き合わせる)。だから鍵と見出しは同じでよい
        self.pick = Some((plain(items), at));
    }

    /// パネルの文字を Python の台本で絵にして、画像としてシートに浮かべる。
    /// writer の方式(図は Python で描いて画像で貼る)の自動化 —
    /// 方程式(EQ_PY)とテキストアート(TEXTART_PY)が同じ道を通る。
    pub(crate) fn insert_py_image(
        &mut self,
        script: &'static str,
        name: &'static str,
        tex: String,
        cx: &mut Context<Self>,
    ) {
        let esc = |t: &str| t.replace('\\', "\\\\").replace('"', "\\\"");
        let dir =
            workdir(name);
        let out = dir.join("eq.png");
        let font = kumihan::font::for_document(None)
            .ok()
            .map(|(fam, _)| fam.path.to_string_lossy().to_string())
            .unwrap_or_default();
        let json = format!(
            "{{\"tex\":\"{}\",\"font\":\"{}\",\"out\":\"{}\"}}",
            esc(&tex),
            esc(&font),
            esc(&out.to_string_lossy())
        );
        let at = self.cursor;
        self.status = ui::t!("typesetting").into();
        let task = cx.background_executor().spawn(async move {
            let _ = std::fs::create_dir_all(&dir);
            let json_path = dir.join("eq.json");
            let py_path = dir.join("eq.py");
            std::fs::write(&json_path, json).map_err(|e| e.to_string())?;
            std::fs::write(&py_path, script).map_err(|e| e.to_string())?;
            let o = std::process::Command::new(find_python())
                .arg(&py_path)
                .arg(&json_path)
                .output()
                .map_err(|e| format!("Python が起動できません: {e}"))?;
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("原因不明");
                return Err(if err.contains("No module named") {
                    format!("matplotlib がありません({last})")
                } else {
                    format!("式が読めません: {last}")
                });
            }
            std::fs::read(&out).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(data) => {
                        let (w, h) = image_px(&data).unwrap_or((200, 60));
                        this.checkpoint();
                        // 200dpi で描いたので画面では半分の大きさに置く
                        this.sheet_mut().images_new.push(book::SheetImage {
                            at,
            dx_px: 0.0,
            dy_px: 0.0,
                            width_px: w as f32 / 2.0,
                            height_px: h as f32 / 2.0,
                            data,
                        });
                        this.dirty = true;
                        this.status = ui::tf!(
                            "placed_image_goes_into_xlsx",
                            // **中の語も訳を通す。** ここだけ素の字だと、
                            // 文は訳されるのに「方程式」が日本語で残ります
                            if name == "eq" { ui::t!("equation") } else { ui::t!("text_art") },
                            at.a1()
                        )
                        .into();
                    }
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// SmartArt を図形の集まりとして組む。1手 = checkpoint 一回(全部まとめて
    /// Ctrl+Z で戻る)。各図形は普通の図形なので、選んで動かす・Enter で
    /// 文字を書く・Del で消す、が全部効く。xlsx へは prstGeom の図形として
    /// 入る(Excel でも図形として見える。本物の SmartArt 部品ではない)。
    pub(crate) fn insert_smartart(&mut self, name: &str, key: &str) {
        self.checkpoint();
        let at = self.cursor;
        let (g, l) = (Some("D5E8DC".to_string()), Some("1B6E3C".to_string()));
        // (dx, dy, w, h, kind, 塗り?, 文字?)
        let mut parts: Vec<(f32, f32, f32, f32, &str, bool, bool)> = Vec::new();
        match key {
            "block-list" => {
                for i in 0..3 {
                    parts.push((i as f32 * 140.0, 0.0, 128.0, 72.0, "roundRect", true, true));
                }
            }
            "vbox-list" => {
                for i in 0..3 {
                    parts.push((0.0, i as f32 * 60.0, 240.0, 48.0, "rect", true, true));
                }
            }
            "pyramid-list" => {
                for i in 0..3 {
                    let w = 160.0 + i as f32 * 60.0;
                    parts.push(((280.0 - w) / 2.0, i as f32 * 58.0, w, 48.0, "roundRect", true, true));
                }
            }
            "basic-process" => {
                for i in 0..3 {
                    parts.push((i as f32 * 164.0, 0.0, 120.0, 56.0, "rect", true, true));
                    if i < 2 {
                        parts.push((i as f32 * 164.0 + 124.0, 16.0, 36.0, 24.0, "rightArrow", true, false));
                    }
                }
            }
            "chevron-process" => {
                for i in 0..3 {
                    parts.push((i as f32 * 140.0, 0.0, 150.0, 56.0, "rightArrow", true, true));
                }
            }
            "timeline" => {
                parts.push((0.0, 46.0, 420.0, 3.0, "rect", true, false)); // 軸
                for i in 0..3 {
                    parts.push((30.0 + i as f32 * 150.0, 38.0, 18.0, 18.0, "ellipse", true, false));
                    parts.push((6.0 + i as f32 * 150.0, 0.0, 100.0, 32.0, "rect", false, true));
                }
            }
            "basic-cycle" => {
                parts.push((110.0, 0.0, 110.0, 64.0, "ellipse", true, true));
                parts.push((0.0, 110.0, 110.0, 64.0, "ellipse", true, true));
                parts.push((220.0, 110.0, 110.0, 64.0, "ellipse", true, true));
            }
            "block-cycle" => {
                parts.push((105.0, 0.0, 120.0, 48.0, "rect", true, true));
                parts.push((220.0, 78.0, 120.0, 48.0, "rect", true, true));
                parts.push((105.0, 156.0, 120.0, 48.0, "rect", true, true));
                parts.push((0.0, 78.0, 120.0, 48.0, "rect", true, true));
            }
            "org-chart" | "hierarchy" => {
                let kids = if key == "org-chart" { 3 } else { 2 };
                let (w, gap) = (120.0, 40.0);
                let total = kids as f32 * w + (kids - 1) as f32 * gap;
                parts.push(((total - w) / 2.0, 0.0, w, 48.0, "rect", true, true));
                // 継ぎの線(細い棒): 親の下 → 横橋 → 子の上
                parts.push((total / 2.0 - 1.0, 48.0, 2.0, 22.0, "rect", true, false));
                parts.push((w / 2.0, 70.0, total - w, 2.0, "rect", true, false));
                for i in 0..kids {
                    let x = i as f32 * (w + gap);
                    parts.push((x + w / 2.0 - 1.0, 72.0, 2.0, 22.0, "rect", true, false));
                    parts.push((x, 94.0, w, 48.0, "rect", true, true));
                }
            }
            "venn" => {
                parts.push((0.0, 0.0, 150.0, 150.0, "ellipse", false, true));
                parts.push((90.0, 0.0, 150.0, 150.0, "ellipse", false, true));
                parts.push((45.0, 78.0, 150.0, 150.0, "ellipse", false, true));
            }
            "matrix" => {
                for r in 0..2 {
                    for c in 0..2 {
                        parts.push((c as f32 * 132.0, r as f32 * 62.0, 124.0, 54.0, "rect", true, true));
                    }
                }
            }
            "pyramid" => {
                for i in 0..3 {
                    let w = 120.0 + i as f32 * 90.0;
                    parts.push(((300.0 - w) / 2.0, i as f32 * 54.0, w, 48.0, "rect", true, true));
                }
            }
            _ => {}
        }
        let n = parts.len();
        for (dx, dy, w, h, kind, filled, texted) in parts {
            self.sheet_mut().shapes_new.push(book::SheetShape {
                at,
                dx_px: dx,
                dy_px: dy,
                width_px: w,
                height_px: h,
                kind: kind.into(),
                fill: if filled { g.clone() } else { None },
                line: l.clone(),
                text: if texted { Some(ui::t!("text").into()) } else { None },
                ..Default::default()
            });
        }
        self.dirty = true;
        self.status = ui::tf!(
            "placed_shapes_select_shape",
            name,
            at.a1(),
            n
        )
        .into();
    }

    /// ソルバーを解く。係数は**表の複製の上で測る**(ゴールシークと同じ流儀):
    /// 変数を全部 0 → 単位ベクトル、で目的と制約左辺の一次係数を取り、
    /// 全部 1 の点で検算して**線形でなければ正直に断る**(単体法 LP は
    /// 線形の問題だけ — 本家 ONLYOFFICE の断り書きと同じ)。
    /// 解くのは scipy.optimize.linprog(highs)。
    pub(crate) fn solve_solver(&mut self, cx: &mut Context<Self>) {
        let Some(sv) = &self.solver else { return };
        // ---- 読み取りと検め ----
        let Some(target) = Pos::parse(&sv.target.text().replace('$', "").to_uppercase()) else {
            self.status = ui::t!("cant_parse_target_cell").into();
            return;
        };
        let Some(vars) = parse_cell_list(sv.vars.text(), 64) else {
            self.status = ui::t!("cant_parse_variable_cells").into();
            return;
        };
        let mode = sv.mode;
        let want = if mode == 2 {
            match sv.value.text().trim().parse::<f64>() {
                Ok(v) => v,
                Err(_) => {
                    self.status = ui::t!("target_value_not_number").into();
                    return;
                }
            }
        } else {
            0.0
        };
        // 制約: (セル, op, 右辺の数)。左辺は範囲なら1セルずつの行になる。
        //
        // **整数・バイナリはここに積みません**(2026-08-21)。あれは
        // 「この変数は整数」という*変数の性質*で、不等式の行ではないからです。
        // HiGHS の `integrality` と `bounds` に渡します。
        let mut rows: Vec<(Pos, usize, f64)> = Vec::new();
        // 変数ごとの整数の印(0=普通 1=整数)と、バイナリかどうか
        let mut int_of: Vec<u8> = vec![0; vars.len()];
        let mut bin_of: Vec<bool> = vec![false; vars.len()];
        for (l, op, r) in &sv.cons {
            let Some(cells) = parse_cell_list(l, 256) else {
                self.status = ui::tf!("cant_read_left_side", l).into();
                return;
            };
            let opi = SOLVER_OPS.iter().position(|o| o == op).unwrap_or(0);
            if opi >= 3 {
                // **左辺は変数セルでなければなりません。** 変数でないセルを
                // 整数にしても、動かす先がないので意味がありません
                for p in &cells {
                    let Some(vi) = vars.iter().position(|v| v == p) else {
                        self.status = ui::tf!(
                            "not_variable_cell_integer",
                            p.a1()
                        )
                        .into();
                        return;
                    };
                    int_of[vi] = 1;
                    if opi == 4 {
                        bin_of[vi] = true;
                    }
                }
                continue;
            }
            // 右辺: 数か、セルの今の値
            let rhs = match r.trim().parse::<f64>() {
                Ok(v) => v,
                Err(_) => match Pos::parse(&r.replace('$', "").to_uppercase()) {
                    Some(p) => self
                        .sheet()
                        .get(p)
                        .map(|c| c.value.as_number())
                        .unwrap_or(0.0),
                    None => {
                        self.status = ui::tf!("cant_read_right_side", r).into();
                        return;
                    }
                },
            };
            for c in cells {
                rows.push((c, opi, rhs));
            }
        }
        // ---- 係数の抽出(表の複製で測る)----
        let base = self.sheet().clone();
        let eval = |xs: &[f64]| -> (f64, Vec<f64>) {
            let mut s = base.clone();
            for (i, p) in vars.iter().enumerate() {
                s.set(*p, Cell::input(&format!("{}", xs[i])));
            }
            recalc(&mut s);
            let g = |p: Pos| s.get(p).map(|c| c.value.as_number()).unwrap_or(0.0);
            (g(target), rows.iter().map(|(p, _, _)| g(*p)).collect())
        };
        let n = vars.len();
        let zeros = vec![0.0; n];
        let (f0, c0) = eval(&zeros);
        let mut obj = vec![0.0; n];
        let mut a: Vec<Vec<f64>> = vec![vec![0.0; n]; rows.len()];
        for i in 0..n {
            let mut xs = zeros.clone();
            xs[i] = 1.0;
            let (fi, ci) = eval(&xs);
            obj[i] = fi - f0;
            for (k, v) in ci.iter().enumerate() {
                a[k][i] = v - c0[k];
            }
        }
        // 線形の検算(全部 1 の点)
        let ones = vec![1.0; n];
        let (f1, c1) = eval(&ones);
        let lin = |measured: f64, base: f64, coefs: &[f64]| -> bool {
            let predicted = base + coefs.iter().sum::<f64>();
            (measured - predicted).abs() <= 1e-6 * measured.abs().max(1.0)
        };
        let mut linear = lin(f1, f0, &obj);
        for k in 0..rows.len() {
            linear = linear && lin(c1[k], c0[k], &a[k]);
        }
        if !linear {
            self.status =
                ui::t!("not_linear_simplex_lp").into();
            return;
        }
        // ---- LP に組む ----
        // 目的: 最大=係数を負に、最小=そのまま、値=目的0で f=want を等式に
        let mut aub: Vec<Vec<f64>> = Vec::new();
        let mut bub: Vec<f64> = Vec::new();
        let mut aeq: Vec<Vec<f64>> = Vec::new();
        let mut beq: Vec<f64> = Vec::new();
        for (k, (_, opi, rhs)) in rows.iter().enumerate() {
            let row = a[k].clone();
            let b = rhs - c0[k];
            match opi {
                0 => {
                    aub.push(row);
                    bub.push(b);
                }
                1 => {
                    aeq.push(row);
                    beq.push(b);
                }
                _ => {
                    aub.push(row.iter().map(|v| -v).collect());
                    bub.push(-b);
                }
            }
        }
        let c: Vec<f64> = match mode {
            0 => obj.iter().map(|v| -v).collect(),
            1 => obj.clone(),
            _ => {
                aeq.push(obj.clone());
                beq.push(want - f0);
                vec![0.0; n]
            }
        };
        // ---- JSON → scipy ----
        let arr = |v: &[f64]| {
            v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(",")
        };
        let mat = |m: &[Vec<f64>]| {
            m.iter().map(|r| format!("[{}]", arr(r))).collect::<Vec<_>>().join(",")
        };
        // **整数の印と枠**(2026-08-21)。バイナリは 0〜1 の枠つきの整数です。
        // 枠は変数ごとに [下, 上] で渡し、`null` は「制限なし」です
        let ints = int_of.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
        let lo = if sv.nonneg { "0" } else { "null" };
        let bounds = bin_of
            .iter()
            .map(|b| if *b { "[0,1]".to_string() } else { format!("[{lo},null]") })
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            "{{\"c\":[{}],\"aub\":[{}],\"bub\":[{}],\"aeq\":[{}],\"beq\":[{}],\
             \"nonneg\":{},\"integrality\":[{}],\"bounds\":[{}]}}",
            arr(&c),
            mat(&aub),
            arr(&bub),
            mat(&aeq),
            arr(&beq),
            sv.nonneg,
            ints,
            bounds
        );
        let has_integer = int_of.contains(&1);
        let dir = workdir("solver");
        self.status = if has_integer {
            ui::t!("looking_solution_integer_program").into()
        } else {
            ui::t!("solving_simplex_lp").into()
        };
        let task = cx.background_executor().spawn(async move {
            let _ = std::fs::create_dir_all(&dir);
            let json_path = dir.join("solver.json");
            let py_path = dir.join("solver.py");
            std::fs::write(&json_path, json).map_err(|e| e.to_string())?;
            std::fs::write(&py_path, SOLVER_PY).map_err(|e| e.to_string())?;
            let o = std::process::Command::new(find_python())
                .arg(&py_path)
                .arg(&json_path)
                .output()
                .map_err(|e| format!("Python が起動できません: {e}"))?;
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("原因不明");
                return Err(if err.contains("No module named") {
                    format!("scipy がありません({last})。次で入ります:\n  {}",
                            pyrun::pip_hint("scipy"))
                } else {
                    last.to_string()
                });
            }
            Ok(String::from_utf8_lossy(&o.stdout).to_string())
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            let _ = this.update(cx, |this, cx| {
                match r {
                    Ok(out) => {
                        let xs: Vec<f64> = out
                            .split('\u{1f}')
                            .filter_map(|v| v.trim().parse().ok())
                            .collect();
                        if xs.len() != vars.len() {
                            this.status = ui::tf!("answer_wrong_shape", out).into();
                        } else {
                            this.checkpoint();
                            for (p, x) in vars.iter().zip(&xs) {
                                let x = (x * 1e9).round() / 1e9;
                                let fmt = this
                                    .sheet()
                                    .get(*p)
                                    .map(|c| c.fmt.clone())
                                    .unwrap_or_default();
                                let mut cell = Cell::input(&format!("{x}"));
                                cell.fmt = fmt;
                                this.book.sheets[this.active].set(*p, cell);
                            }
                            recalc_book(&mut this.book, this.active);
                            this.dirty = true;
                            this.sync_input();
                            this.solver = None;
                            let got = this
                                .sheet()
                                .get(target)
                                .map(|c| c.value.display())
                                .unwrap_or_default();
                            this.status = ui::tf!(
                                "solved_variable_cells_rewritten",
                                target.a1(),
                                got,
                                xs.len()
                            )
                            .into();
                        }
                    }
                    Err(e) => this.status = e.into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// ゴールシーク。変えるセルの値を割線法で探す(表の複製の上で試す)。
    pub(crate) fn goal_seek(&mut self, target: Pos, goal: f64, var: Pos) {
        let base = self.sheet().clone();
        if base.get(target).and_then(|c| c.formula.as_ref()).is_none() {
            self.status = ui::tf!("not_formula_cell", target.a1()).into();
            return;
        }
        let found = solve_goal(&base, target, goal, var);
        match found {
            Some(x) => {
                let x = (x * 1e9).round() / 1e9;
                self.checkpoint();
                let fmt = self.sheet().get(var).map(|c| c.fmt.clone()).unwrap_or_default();
                let mut cell = Cell::input(&format!("{x}"));
                cell.fmt = fmt;
                self.sheet_mut().set(var, cell);
                recalc_book(&mut self.book, self.active);
                self.dirty = true;
                self.sync_input();
                self.status = ui::tf!(
                    "becomes_ctrl_z_undoes",
                    var.a1(),
                    x,
                    target.a1(),
                    goal
                )
                .into();
            }
            None => {
                self.status = ui::tf!(
                    "no_solution_found_may",
                    var.a1(),
                    target.a1()
                )
                .into();
            }
        }
    }
}

impl Calc {
    /// データテーブル(感度表)。選んだ矩形の縁に置いた入力値を差し替えながら
    /// 式を計算し、中身を**値として**埋める。Excel の作法に合わせる:
    /// - 1変数(列): 左の列に入力値、上の行に式、角は空
    /// - 2変数: **角が式**、左の列と上の行に入力値
    ///
    /// 本家は TABLE() の配列式で後から追随するが、うちは**その時の値**
    /// (ピボットと同じ割り切り)。入力を直したらもう一度押す。
    /// 計算はブックの複製の上でやるので、途中の値が本物に混ざらない
    pub(crate) fn data_table(&mut self, col_in: Option<Pos>, row_in: Option<Pos>) {
        let (a, b) = self.sel_rect();
        if a.row >= b.row || a.col >= b.col {
            self.status =
                ui::t!("select_rectangular_range_holding").into();
            return;
        }
        let si = self.active;
        let two = row_in.is_some() && col_in.is_some();
        // 埋める先と、そのとき差し替える入力の組
        let mut jobs: Vec<MergeJob> = Vec::new();
        if two {
            let (ci, ri) = (col_in.unwrap(), row_in.unwrap());
            // 角(a)が式。左の列 = 列の入力、上の行 = 行の入力
            if self.sheet().get(a).and_then(|c| c.formula.as_ref()).is_none() {
                self.status =
                    ui::tf!("two_variables_corner_formula", a.a1()).into();
                return;
            }
            for r in (a.row + 1)..=b.row {
                for c in (a.col + 1)..=b.col {
                    let cv = self.sheet().get(Pos::new(r, a.col)).map(|x| x.editable()).unwrap_or_default();
                    let rv = self.sheet().get(Pos::new(a.row, c)).map(|x| x.editable()).unwrap_or_default();
                    if cv.is_empty() || rv.is_empty() {
                        continue;
                    }
                    jobs.push((Pos::new(r, c), vec![(ci, cv), (ri, rv)], a));
                }
            }
        } else {
            let Some(ci) = col_in.or(row_in) else {
                self.status = ui::t!("type_input_cell_b2").into();
                return;
            };
            // 1変数(列): 上の行の式ごとに、左の列の値を差し替える
            for r in (a.row + 1)..=b.row {
                let cv = self.sheet().get(Pos::new(r, a.col)).map(|x| x.editable()).unwrap_or_default();
                if cv.is_empty() {
                    continue;
                }
                for c in (a.col + 1)..=b.col {
                    let f = Pos::new(a.row, c);
                    if self.sheet().get(f).and_then(|x| x.formula.as_ref()).is_none() {
                        continue;
                    }
                    jobs.push((Pos::new(r, c), vec![(ci, cv.clone())], f));
                }
            }
        }
        if jobs.is_empty() {
            self.status = ui::t!(
                "nothing_fill_formula_goes"
            )
            .into();
            return;
        }
        // 複製の上で回す(本物は最後に1手で書き換える)
        let mut work = self.book.clone();
        let mut out: Vec<(Pos, book::Value)> = Vec::new();
        for (dest, inputs, f) in &jobs {
            for (p, v) in inputs {
                let fmt = work.sheets[si].get(*p).map(|c| c.fmt.clone()).unwrap_or_default();
                let mut cell = book::Cell::input(v);
                cell.fmt = fmt;
                work.sheets[si].set(*p, cell);
            }
            recalc_book(&mut work, si);
            out.push((*dest, work.sheets[si].value(*f)));
        }
        self.checkpoint();
        for (p, v) in &out {
            let fmt = self.sheet().get(*p).map(|c| c.fmt.clone()).unwrap_or_default();
            let mut cell = book::Cell::input(&v.display());
            cell.fmt = fmt;
            self.sheet_mut().set(*p, cell);
        }
        recalc_book(&mut self.book, si);
        self.dirty = true;
        self.sync_input();
        self.status = ui::tf!(
            "data_table_filled_answers",
            out.len(),
            if two { ui::t!("two_variables") } else { ui::t!("one_variable") }
        )
        .into();
    }
}

/// PDF の台本の返しを (ページ, 取り方, セル) の並びにする。
///
/// 形は台本と揃えた区切り文字づけです。**読めない物が来たら空を返します** —
/// 半端に読めた表を出すと、そこから数字がずれます。
pub(crate) fn parse_pdf_tables(raw: &str) -> Vec<(u32, String, Vec<Vec<String>>)> {
    let raw = raw.trim_end_matches(['\n', '\r']);
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split('\u{1d}')
        .filter_map(|t| {
            let (head, body) = t.split_once('\u{1e}')?;
            let (page, how) = head.split_once('\u{1f}')?;
            let page: u32 = page.trim().parse().ok()?;
            let rows: Vec<Vec<String>> = body
                .split('\u{1e}')
                .map(|r| r.split('\u{1f}').map(|c| c.to_string()).collect())
                .collect();
            (!rows.is_empty()).then_some((page, how.to_string(), rows))
        })
        .collect()
}

/// 選んだ範囲の形(推奨グラフの判断に使う)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RangeShape {
    /// 項目(行)の数。見出し行は数えない
    pub(crate) points: usize,
    /// 系列(2列目以降)の数
    pub(crate) series: usize,
    /// 1列目が全部数(項目名でなく X の値)
    pub(crate) first_col_numeric: bool,
    /// 1列目が日付・年・月に見える
    pub(crate) first_col_time: bool,
    /// 値に負の数がある(円グラフには向かない)
    pub(crate) has_negative: bool,
}

/// グラフの種類(chart.py の kind)と、一覧の鍵。並びは手引きの表と同じ
pub(crate) const CHART_KINDS: &[(&str, &str)] = &[
    ("bar", "chart_column"),
    ("barh", "chart_bar"),
    ("line", "chart_line"),
    ("area", "chart_area"),
    ("pie", "chart_pie"),
    ("scatter", "chart_scatter"),
];

/// 一覧に出す(鍵, 見出し)。`ui::t!` は literal しか受けないので、ここで結ぶ
fn chart_kind_item(kind: &str) -> (&'static str, SharedString) {
    match kind {
        "barh" => ("chart_bar", ui::t!("chart_bar").into()),
        "line" => ("chart_line", ui::t!("chart_line").into()),
        "area" => ("chart_area", ui::t!("chart_area").into()),
        "pie" => ("chart_pie", ui::t!("chart_pie").into()),
        "scatter" => ("chart_scatter", ui::t!("chart_scatter").into()),
        _ => ("chart_column", ui::t!("chart_column").into()),
    }
}

/// 「2024」「4月」「Q1」「2024/04」のような、時の流れに見える項目名か
pub(crate) fn looks_like_time_label(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() {
        return false;
    }
    if t.ends_with('月') || t.ends_with('年') || t.ends_with("年度") {
        return true;
    }
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let up = t.to_ascii_uppercase();
    if up.len() == 2 && up.starts_with('Q') && up[1..].chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // 2024/04・2024-04-01 のような区切りつきの数
    let parts: Vec<&str> = t.split(['/', '-', '.']).collect();
    parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// 範囲の形から、合う種類を合う順に並べる。
///
/// 決めは手引きの表のとおりです:
/// * 1列目が全部数なら散布図を先頭に
/// * 時系列か 13 行以上なら折れ線・面・縦棒
/// * 1系列で 2〜8 行、負の数が無ければ円・縦棒・横棒・折れ線
/// * それ以外は縦棒・横棒・折れ線・面
/// * 横棒は 20 行まで(それより多いと読めない)
pub(crate) fn recommended_kinds(sh: RangeShape) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    let mut push = |k: &'static str| {
        if !v.contains(&k) {
            v.push(k);
        }
    };
    let barh_ok = sh.points <= 20;
    let pie_ok = sh.series == 1 && (2..=8).contains(&sh.points) && !sh.has_negative;
    if sh.first_col_numeric && sh.series >= 1 {
        push("scatter");
    }
    if sh.first_col_time || sh.points > 12 {
        push("line");
        push("area");
        push("bar");
        if barh_ok {
            push("barh");
        }
    } else if pie_ok {
        push("pie");
        push("bar");
        push("barh");
        push("line");
        push("area");
    } else {
        push("bar");
        if barh_ok {
            push("barh");
        }
        push("line");
        push("area");
    }
    v
}

#[cfg(test)]
mod chart_tests {
    use super::*;

    fn shape(points: usize, series: usize) -> RangeShape {
        RangeShape { points, series, first_col_numeric: false, first_col_time: false, has_negative: false }
    }

    #[test]
    fn one_short_series_recommends_pie_first() {
        assert_eq!(recommended_kinds(shape(5, 1)), vec!["pie", "bar", "barh", "line", "area"]);
    }

    #[test]
    fn negative_values_never_recommend_pie() {
        let mut s = shape(5, 1);
        s.has_negative = true;
        assert!(!recommended_kinds(s).contains(&"pie"));
    }

    #[test]
    fn many_points_or_time_labels_recommend_line_first() {
        assert_eq!(recommended_kinds(shape(30, 2)), vec!["line", "area", "bar"]);
        let mut s = shape(6, 2);
        s.first_col_time = true;
        assert_eq!(recommended_kinds(s), vec!["line", "area", "bar", "barh"]);
    }

    #[test]
    fn numeric_first_column_puts_scatter_first() {
        let mut s = shape(6, 2);
        s.first_col_numeric = true;
        assert_eq!(recommended_kinds(s)[0], "scatter");
    }

    #[test]
    fn plain_table_recommends_column_first() {
        assert_eq!(recommended_kinds(shape(6, 3)), vec!["bar", "barh", "line", "area"]);
    }

    #[test]
    fn every_recommended_kind_has_a_list_key() {
        for sh in [shape(5, 1), shape(30, 2), shape(6, 3)] {
            for k in recommended_kinds(sh) {
                assert!(CHART_KINDS.iter().any(|(kind, _)| *kind == k), "{k} に鍵が無い");
            }
        }
    }

    #[test]
    fn time_labels_are_recognised() {
        for t in ["2024", "4月", "2024年", "Q1", "2024/04", "2024-04-01"] {
            assert!(looks_like_time_label(t), "{t}");
        }
        for t in ["東京", "A", "12345", "", "りんご"] {
            assert!(!looks_like_time_label(t), "{t}");
        }
    }
}
