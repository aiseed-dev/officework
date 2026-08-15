//! writer の Python の裏方(main.rs から純移動 2026-08-08。部屋割りの4歩目)。
//! Python を探す・マクロの台本を組む・プラグインの置き場。
//! 探す・置き場は 2026-08-12 に pyrun へ(calc と同じ物を使う)。

/// Python の探し方(calc と同じ — 正は pyrun)。JO_PYTHON > .venv > python3。
/// 前はここに写しがあり、**実行ファイルから遡って .venv を探す直し
/// (2026-08-07)が入っていなかった** — 写経のずれの実物。共有で自然に直る
pub(crate) use pyrun::find_python;

/// マクロ台本の全文を組む。前置きで d(python-docx の文書)のほかに、
/// 記入欄へ**名前で**書く fill / fill_one、読む extract、名前と値の一覧
/// fields を渡す(記入=出口と吸い上げ=入口の対)。鍵は docx の w:tag
/// (フォームタブの「名前」ボタンで付ける)。無い名前は例外で断る —
/// ラベルの字を探して隣に書く走査より、名前が様式の背骨(発注者 2026-08-05)。
/// もう一車線が雛形: render(辞書)= docxtpl の {{名前}} / {%tr %} 差し込み、
/// tpl_fields()= 差し込み口の一覧。往復する様式は記入欄、出して終わりの
/// 量産文書(通知書・契約書)は雛形、の使い分け(SEKKEI 参照)
pub(crate) fn macro_script(
    in_d: &std::path::Path,
    out_d: &std::path::Path,
    user_code: &str,
) -> String {
    // 記入の道具。lxml の Element は「子が無い=偽」なので is None で判定する
    const FILL: &str = r#"from docx.oxml.ns import qn
def _sdts(name):
    es = []
    for sdt in d.element.iter(qn('w:sdt')):
        pr = sdt.find(qn('w:sdtPr'))
        tag = pr.find(qn('w:tag')) if pr is not None else None
        if tag is None:
            continue
        t = tag.get(qn('w:val')) or ''
        # 「jo:email:連絡先」= writer 独自の種類の印+名前。名前でも引ける
        if t == name or (t.startswith('jo:') and t.split(':', 2)[-1] == name):
            es.append(sdt)
    return es
def _put(sdt, value):
    ct = sdt.find(qn('w:sdtContent'))
    if ct is None:
        raise SystemExit('記入欄の中身がありません')
    ts = list(ct.iter(qn('w:t')))
    v = str(value)
    if ts:
        ts[0].text = v
        for t in ts[1:]:
            t.text = ''
    else:
        r = ct.find('.//' + qn('w:r'))
        if r is None:
            p = ct.find('.//' + qn('w:p'))
            parent = ct if p is None else p
            r = parent.makeelement(qn('w:r'), {})
            parent.append(r)
        t = r.makeelement(qn('w:t'), {})
        t.text = v
        r.append(t)
def fill(name, value):
    # 同じ名前の欄すべてに書く(表紙と2枚目に同じ欄がある様式のため)
    es = _sdts(name)
    if not es:
        raise SystemExit('記入欄「%s」が見つかりません(writer のフォームタブ「名前」で付けます)' % name)
    for e in es:
        _put(e, value)
    return len(es)
def fill_one(name, value):
    # 最初の一つにだけ書く
    es = _sdts(name)
    if not es:
        raise SystemExit('記入欄「%s」が見つかりません' % name)
    _put(es[0], value)
def _text(sdt):
    ct = sdt.find(qn('w:sdtContent'))
    if ct is None:
        return ''
    return ''.join(t.text or '' for t in ct.iter(qn('w:t')))
def extract(name):
    # 記入欄の値を読む(同じ名前が複数なら最初の一つ)。無い名前は断る
    es = _sdts(name)
    if not es:
        raise SystemExit('記入欄「%s」が見つかりません' % name)
    return _text(es[0])
def fields():
    # 名前つき記入欄の(名前, 値)の一覧 — 様式の仕様書。同じ名前は欄ごとに並ぶ
    out = []
    for sdt in d.element.iter(qn('w:sdt')):
        pr = sdt.find(qn('w:sdtPr'))
        tag = pr.find(qn('w:tag')) if pr is not None else None
        if tag is None:
            continue
        t = tag.get(qn('w:val')) or ''
        if t.startswith('jo:'):
            t = t.split(':', 2)[-1]
            if t in ('email', 'phone', 'complex', 'signature'):
                continue  # 種類の印だけ(名前なし)の欄
        if t:
            out.append((t, _text(sdt)))
    return out
def _tpl():
    try:
        from docxtpl import DocxTemplate
    except ImportError:
        raise SystemExit('docxtpl がありません(pip install docxtpl。.venv があればそちらへ)')
    return DocxTemplate(IN)
def render(ctx):
    # 雛形({{名前}} と {%tr for %} の行くり返し)に辞書を差し込む。
    # 以後の d は差し込み済みの文書になり、そのまま保存される
    global d
    t = _tpl()
    try:
        t.render(ctx)
    except Exception as e:
        raise SystemExit('雛形が壊れています(タグの置き方や全角の {{ }} を確かめてください): %s' % e)
    d = t.docx
    return d
def tpl_fields():
    # 雛形の差し込み口({{名前}})の一覧 — 雛形の仕様書。壊れていれば断る
    t = _tpl()
    try:
        return sorted(t.get_undeclared_template_variables())
    except Exception as e:
        raise SystemExit('雛形が壊れています: %s' % e)
"#;
    format!(
        concat!(
            "import docx\n",
            "IN = {in_d:?}\n",
            "d = docx.Document(IN)\n",
            "{fill}",
            "# ---- 利用者のコード(d = python-docx の文書 / fill(名前, 値)・\
             extract(名前)・fields() = 記入欄 / render(辞書)・tpl_fields() = \
             {{{{ }}}} の雛形) ----\n",
            "{code}\n",
            "# ----\n",
            "d.save({out_d:?})\n"
        ),
        in_d = in_d.to_string_lossy(),
        fill = FILL,
        out_d = out_d.to_string_lossy(),
        code = user_code
    )
}

/// プラグイン(.py)の置き場。~/.config/officework/plugins(正は pyrun)
pub(crate) use pyrun::plugins_dir;

/// 数式を組む台本。**officework/tex.py をそのまま埋め込む** —
/// 写しを作らない(py.rs の冒頭が戒めている「写経のずれ」を作らないため)。
/// 実体は一つで、Python からは `from officework import tex`、
/// アプリからはこの埋め込みで使う。**officework が入っていなくても動く**
/// (calc の CHART_PY などと同じで、要るのは matplotlib だけ)
const TEX_PY: &str = include_str!("../../pysheet/officework/tex.py");

/// 埋め込んだ tex.py の後ろに付ける呼び出し。argv は (式, 大きさ, 出力先)
const SUUSHIKI_YOBI: &str = r#"
# ---- ここから writer の呼び出し ----
import json as _json, sys as _sys
try:
    _png, _w, _h = to_png(_sys.argv[1], size_pt=float(_sys.argv[2]))
except Muri as _e:
    print(_json.dumps({"e": str(_e)})); raise SystemExit(0)
except Exception as _e:
    print(_json.dumps({"e": "%s: %s" % (type(_e).__name__, _e)})); raise SystemExit(0)
open(_sys.argv[3], "wb").write(_png)
print(_json.dumps({"w": _w, "h": _h, "kata": kumi_kata()}))
"#;

/// いま数式を何で組むか(状態行に出す字)。**組めないなら組めないと言う**
pub(crate) fn suushiki_no_kumi_kata() -> String {
    // **固有名詞は訳さない**(鍵にすると 14 言語ぶんの無駄な行が増える)
    match hashiru_kata() {
        Some(s) if s.trim() == "tex" => "TeX".into(),
        Some(s) if s.trim() == "mathtext" => "matplotlib".into(),
        _ => "Python".into(),
    }
}

/// 囲いの中から officework が見える道(.venv と PYTHONPATH の置き場)。
/// bwrap は /home を tmpfs にするので、見せないと import で落ちる
fn binds_for_python() -> Vec<std::path::PathBuf> {
    let mut v: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::fs::canonicalize(".venv") {
        v.push(p);
    }
    if let Some(pp) = std::env::var_os("PYTHONPATH") {
        for part in std::env::split_paths(&pp) {
            if let Ok(p) = std::fs::canonicalize(&part) {
                v.push(p);
            }
        }
    }
    v
}

/// いま何で組めるかを、埋め込んだ tex.py に聞く(サンドボックスの中)。
fn hashiru_kata() -> Option<String> {
    let py = find_python();
    let dir = pyrun::cage_work_dir("suushiki");
    std::fs::create_dir_all(&dir).ok()?;
    let script = dir.join("kata.py");
    std::fs::write(&script, format!("{TEX_PY}\nprint(kumi_kata())\n")).ok()?;
    let mut c = pyrun::caged_python(&py, &dir, &binds_for_python(), false)?;
    c.arg(&script);
    let out = c.output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    s.lines().last().map(|l| l.to_string())
}

/// **数式を組む。** 打った LaTeX を Python へ渡し、絵と寸法(mm)をもらう。
/// 組めなければ**理由をそのまま**返す(黙って何も起きない、をしない)。
pub(crate) fn kumu_suushiki(tex: &str, size_pt: f32) -> Result<(Vec<u8>, f32, f32), String> {
    let py = find_python();
    let dir = pyrun::cage_work_dir("suushiki");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let script = dir.join("kumu.py");
    std::fs::write(&script, format!("{TEX_PY}{SUUSHIKI_YOBI}"))
        .map_err(|e| e.to_string())?;
    let png = dir.join("out.png");
    let _ = std::fs::remove_file(&png);
    // **囲いの中から officework が見えるようにする。** bwrap は /home を
    // tmpfs にするので、.venv も PYTHONPATH の置き場も見せないと import で落ちる
    // (calc の Python と同じ作法)。組めない機械では素の Python
    let mut c = match pyrun::caged_python(&py, &dir, &binds_for_python(), false) {
        Some(c) => c,
        None => std::process::Command::new(&py),
    };
    c.arg(&script).arg(tex).arg(format!("{size_pt}")).arg(&png);
    let out = c.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let last = stdout.lines().last().unwrap_or("").to_string();
    if let Some(e) = ops::Jobj::parse(&last).and_then(|o| o.str("e")) {
        return Err(e);
    }
    let o = ops::Jobj::parse(&last)
        .ok_or_else(|| ui::tf!("Python の返事が読めません: {}", last).to_string())?;
    let (w, h) = (o.num("w").unwrap_or(0.0) as f32, o.num("h").unwrap_or(0.0) as f32);
    if w <= 0.0 || h <= 0.0 {
        return Err(ui::t!("寸法が返りません").to_string());
    }
    let bytes = std::fs::read(&png).map_err(|e| e.to_string())?;
    Ok((bytes, w, h))
}
