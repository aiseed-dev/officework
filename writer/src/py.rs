//! writer の Python の裏方(main.rs から純移動 2026-08-08。部屋割りの4歩目)。
//! Python を探す・マクロの台本を組む・プラグインの置き場。
//! 探す・置き場は 2026-08-12 に pyrun へ(calc と同じ物を使う)。

/// Python の探し方(calc と同じ — 正は pyrun)。JO_PYTHON > .venv > python3。
/// 前はここに写しがあり、**実行ファイルから遡って .venv を探す直し
/// (2026-08-07)が入っていなかった** — 写経のずれの実物。共有で自然に直る
pub(crate) use pyrun::find_python;

/// Builds the whole macro script. Besides d (the python-docx document), the preamble
/// hands over fill / fill_one, which write into the fill-in fields **by name**, extract,
/// which reads one, and fields, which lists the names and their values (writing out and
/// reading back are a pair). The key is the docx w:tag, which the Name button on the
/// Form tab attaches. A name that does not exist raises an exception. The owner decided
/// on 2026-08-05 that the name is the backbone of the form, rather than scanning for
/// the text of a label and writing next to it.
/// The other way is a template: render(dict) fills in docxtpl's {{member}} and {%tr %},
/// and tpl_fields() lists the placeholders. A form that goes back and forth uses fill-in
/// fields, while documents produced in bulk and sent out (notices, contracts) use a
/// template (see SEKKEI).
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
    # 雛形({{member}} と {%tr for %} の行くり返し)に辞書を差し込む。
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
    # 雛形の差し込み口({{member}})の一覧 — 雛形の仕様書。壊れていれば断る
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

/// What formulas are laid out with right now (the text shown in the status bar).
/// **Fixed to typst in the engine.** The owner decided this on 2026-09-02; before that
/// it was TeX or matplotlib in Python.
pub(crate) fn suushiki_no_kumi_kata() -> String {
    "typst".into()
}

/// **数式を組む。** 打った LaTeX をエンジン(typst + mitex)に渡し、絵と
/// 寸法(mm)をもらう。Python は使わない。`font` は文書の書体の名前で、
/// 数式の中の日本語をその書体で出すために渡す。
/// 組めなければ**理由をそのまま**返す(黙って何も起きない、をしない)。
pub(crate) fn kumu_suushiki(tex: &str, size_pt: f32, font: Option<&str>) -> Result<(Vec<u8>, f32, f32), String> {
    let bytes = kumihan::font::for_document(font)
        .ok()
        .and_then(|(f, _)| kumihan::font::load(f).ok());
    let k = kumihan::suushiki::kumu(tex, size_pt, bytes.as_deref())?;
    Ok((k.png, k.w_mm, k.h_mm))
}
