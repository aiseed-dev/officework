//! Python 実行の機構 — **calc と writer が共に使う**(SEKKEI「操作の言葉を
//! 1本に」段A。2026-08-12 に calc/src/py.rs と ui::pyedit から純移動)。
//!
//! ここにあるのは「どう走らせるか」だけ: サンドボックス(Cage)・時間制限・
//! python の探索・plugins の走査・裏方の台本。**何を走らせるか**(UDF の組み
//! 立て・結果の適用・画面)は各アプリに残る。gpui も sheet も知らない —
//! だからこのクレートは WASM でも sidecar でも組める。
//!
//! 切り出しの前は同じ機構が3回書かれていた: calc/src/py.rs(全部)、
//! writer/src/py.rs(find_python — 2026-08-07 の .venv 探索の直しが
//! **入っていない**まま)、writer/src/doc.rs(生の bwrap — 時間制限も
//! Flatpak の分岐も無い)。**二重に書いた物は必ずずれる**の実物。

use std::path::PathBuf;

// ---- 設定の置き場 ----------------------------------------------------------

/// 設定と控えの置き場。**`~/.config/officework`**。
///
/// # なぜここに在るのか
///
/// 置き場を決める所が散らばると必ずずれる(実際、9箇所が別々に径路を
/// 書いていた)。**1箇所に集める**。pyrun は依存ゼロのいちばん下の層で、
/// lang → face → アプリの全部から見えるので、置き場としてここが都合よい
/// (pyrun の口上「ここに何かを足したくなったら pyrun の仕事ではない
/// 合図」の例外 — これは依存ではなく径路の一言)。
///
/// # 名前を直した経緯(2026-08-16)
///
/// 2026-08-08 に製品名を office → **officework** に改めたのに、設定の
/// 置き場だけ `~/.config/office` のまま残っていた(発注者が気づいた)。
/// **まだ公開前なので、古い名前は残さず移した** — 二重に読む道を作ると、
/// どちらが正かが分からなくなる。
pub fn config_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/officework")
}

// ---- plugins(.py の置き場)-------------------------------------------------

/// **マクロ**(.py)の置き場。`<設定の置き場>/plugins`。
/// 人が一覧から選んだときだけ走る物がここに居る。
/// **ここが正** — ui::pyedit と calc/writer は包みで呼ぶ
pub fn plugins_dir() -> PathBuf {
    config_dir().join("plugins")
}

/// **式から呼ぶ関数(UDF)**の置き場。`<設定の置き場>/funcs`。
///
/// # マクロと分けた理由(2026-08-16 発注者「UDF とマクロに区分しないと
/// いけないのでは」)
///
/// 前は両方が `plugins` に同居していて、**マクロの中の def が全部**
/// 表の関数として登録されていた(補助関数まで)。それだけなら散らかるだけ
/// だが、効くのはその先:
///
/// - **UDF は人が押さなくても走る。** 再計算のたびに呼ばれる
/// - **ブックが名前で呼べる。** `=集計(A1:B9)` は xlsx に保存されるので、
///   受け取ったブックを開くと、こちらの `集計.py` が走る
///
/// 「ブックはコードを運ばない」は守れていても、**ブックはコードの名前を
/// 運ぶ**。だから、式から呼ばれてよい物だけを別の置き場に置く —
/// **そこへ置く手が門**になる(マクロの一覧と同じ形)。
pub fn funcs_dir() -> PathBuf {
    config_dir().join("funcs")
}

/// **操作の記録**の置き場(2026-08-16 発注者「記録は、記録のディレクトリに
/// いれる」)。
///
/// 記録は**下書き**で、マクロは**据えた物**。同じ置き場に落とすと、押した
/// 手がそのまま走る台本になって並ぶ — 読む前に、直す前に。分けておけば
/// 「記録した物」と「これでいいと決めた物」が目で見て分かれる。
pub fn records_dir() -> PathBuf {
    config_dir().join("records")
}

/// **リボンに出るマクロ**の置き場(2026-08-16 発注者「リボン用のマクロは
/// 別にしたほうがいいのでは」)。
///
/// plugins との違いは**名乗り**にある。plugins の .py は何も名乗らなくてよい
/// — 一覧に名前が出て、人が選ぶ。リボンに出るには、札(何と書くか)・
/// 絵(どの印か)・段(どのタブか)を名乗る必要がある。名乗る物と名乗らない
/// 物を同じ置き場に混ぜると、「名乗り忘れ」と「名乗る気が無い」が区別できない。
///
/// 同梱のピボット・グラフ・ソルバー等(下の `*_PY`)は**同じ役目の
/// システム定義**。利用者の定義はここに置く。
pub fn ribbon_dir() -> PathBuf {
    config_dir().join("ribbon")
}

/// **利用者が足したパッケージ**の置き場。`<設定の置き場>/.venv`。
///
/// # なぜ同梱の Python に入れさせないか(2026-08-17)
///
/// 「matplotlib は同梱の python に pip で入れられる」と書いていたが、
/// **3つの配り方すべてで壊れる**と分かった:
///
/// - **macOS**: 署名済みの `.app` の中に後から `.so` を足すと**封が破れる**。
///   同じ日に公証を通したばかりで、正面から衝突していた
/// - **Linux(.deb)**: `/opt/officework` は root 所有。`sudo pip` になる
/// - **Windows**: 入るが、入れ物が更新・削除で `{app}\python` を丸ごと消す
///
/// だから**同梱の Python は読むだけの物**にして、足した物は利用者の側に置く。
/// マクロ(funcs / plugins)と同じ置き場なのは、**エディタの設定を要らなく
/// するため**(発注者 2026-08-17「仮想環境と同じ場所にしないと editor の
/// 設定が面倒」)— `~/.config/officework` を開けば、VS Code も PyCharm も
/// 隣の `.venv` を自分で見つける。綴りを `.venv` にしてあるのはそのため。
pub fn venv_dir() -> PathBuf {
    config_dir().join(".venv")
}

/// 利用者の venv の Python(**在るときだけ**)。無ければ None。
pub fn venv_python() -> Option<PathBuf> {
    let p = if cfg!(windows) {
        venv_dir().join("Scripts/python.exe")
    } else {
        venv_dir().join("bin/python3")
    };
    p.exists().then_some(p)
}

/// 利用者の venv の pip(案内に出す径路。**在らなくても綴りは返す**)。
pub fn venv_pip() -> PathBuf {
    if cfg!(windows) {
        venv_dir().join("Scripts/pip.exe")
    } else {
        venv_dir().join("bin/pip")
    }
}

/// 利用者の venv が**壊れていないか**。
///
/// venv は作った時の Python の径路を `pyvenv.cfg` に焼き付けるので、
/// **アプリを入れ直して径路が変わると動かなくなる**(2026-08-14 に一度
/// 踏んだ)。焼き付いた先が消えていたら作り直す合図。
pub fn venv_broken() -> bool {
    let cfg = venv_dir().join("pyvenv.cfg");
    let Ok(s) = std::fs::read_to_string(&cfg) else {
        return venv_dir().exists(); // 中身はあるのに cfg が無い = 壊れている
    };
    let home = s
        .lines()
        .find_map(|l| l.strip_prefix("home").map(|r| r.trim_start_matches([' ', '=']).trim()));
    match home {
        Some(h) => !std::path::Path::new(h).exists(),
        None => true,
    }
}

/// 利用者の venv を用意する(**在れば何もしない**)。
///
/// 素の Python(同梱か機械の物)から `--system-site-packages` で作るので、
/// 同梱の標準ライブラリと `officework` はそのまま見えて、**足した物だけ**が
/// 利用者の側に載る。壊れていたら畳んで作り直す。
///
/// 返すのは出来上がった Python。作れなければ理由を返す(**黙って諦めない**)。
pub fn ensure_venv() -> Result<PathBuf, String> {
    if let Some(p) = venv_python() {
        if !venv_broken() {
            return Ok(p);
        }
        // 焼き付いた径路が消えている — 畳んで作り直す
        let _ = std::fs::remove_dir_all(venv_dir());
    }
    let base = base_python();
    let out = std::process::Command::new(&base)
        .args(["-m", "venv", "--system-site-packages"])
        .arg(venv_dir())
        .output()
        .map_err(|e| format!("{} を起こせません: {e}", base.display()))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    venv_python().ok_or_else(|| "作った venv に python が居ません".to_string())
}

/// **入れ方の案内**。利用者の venv を用意して、**そのまま打てる1行**を返す。
///
/// 「pip で入れてください」だけでは、**どの pip か**が利用者に分からない
/// (機械に python が何本も入っているのが普通)。ここで径路まで言う。
///
/// 呼ばれるのは「〜がありません」と言う瞬間だけなので、**Python を使わない
/// 人の所には venv を作らない**。作れなかったときは素の `pip` と言う —
/// 動かない径路を見せない。
pub fn pip_hint(pkg: &str) -> String {
    match ensure_venv() {
        Ok(_) => format!("{} install {pkg}", venv_pip().display()),
        Err(_) => format!("pip install {pkg}"),
    }
}

/// リボンに出るマクロの宣言。
pub struct RibbonDecl {
    /// .py の名前(= 走らせるモジュール名)
    pub module: String,
    /// ボタンに出すラベル。**訳しません** — 利用者自身の言葉です
    pub label: String,
    /// アイコンの名前(icons の slot)。無い名前なら呼ぶ側が既定に落とす
    pub icon: String,
    /// どのタブに出すか(既定は「マクロ」)
    pub tab: String,
}

/// 置き場の .py から宣言を読む。**Python は走らせない** — [`def_names`] と
/// 同じで行を読むだけ(宣言を読むために利用者のコードを走らせたら、
/// 「押したときだけ走る」が嘘になる)。
///
/// 宣言の形は普通の Python の辞書1つです:
///
/// ```python
/// リボン = {"ラベル": "月次の締め", "アイコン": "py-list", "タブ": "マクロ"}
/// ```
///
/// キーは英語の綴り(`ribbon` / `label` / `icon` / `tab`)でも書けます。
///
/// **前の書き方も動きます。** はじめは「札」「絵」「段」というキーでしたが、
/// 普通の言葉ではないので言い換えました(2026-08-21)。既に書いた .py が
/// 動かなくなると困るので、古いキーも読み続けます。
///
/// 宣言の無い .py はリボンに出ない(置き忘れをボタンにしない)。
pub fn ribbon_decls(dir: &std::path::Path) -> Vec<RibbonDecl> {
    modules_in(dir)
        .into_iter()
        .filter_map(|m| {
            let src = std::fs::read_to_string(dir.join(format!("{m}.py"))).ok()?;
            let kv = decl_dict(&src)?;
            let get = |names: &[&str]| {
                kv.iter()
                    .find(|(k, _)| names.contains(&k.as_str()))
                    .map(|(_, v)| v.clone())
            };
            Some(RibbonDecl {
                label: get(&["ラベル", "label", "札"]).unwrap_or_else(|| m.clone()),
                icon: get(&["アイコン", "icon", "絵"]).unwrap_or_default(),
                tab: get(&["タブ", "tab", "段"]).unwrap_or_else(|| "マクロ".into()),
                module: m,
            })
        })
        .collect()
}

/// `リボン = { … }` の中の `"鍵": "値"` を拾う。**浅い読み手**で足りる —
/// 名乗りは1段の辞書で、入れ子も式も要らない(rpc.rs の JSON と同じ流儀。
/// 依存を増やさない)。見つからなければ `None`
fn decl_dict(src: &str) -> Option<Vec<(String, String)>> {
    let head = src.match_indices('{').find(|(i, _)| {
        let before = src[..*i].trim_end();
        before.strip_suffix('=').is_some_and(|b| {
            let b = b.trim_end();
            b.ends_with("リボン") || b.ends_with("ribbon")
        })
    })?;
    let rest = &src[head.0 + 1..];
    let end = rest.find('}')?;
    let body = &rest[..end];
    let mut out = Vec::new();
    let mut it = body.char_indices().peekable();
    let mut cur: Vec<String> = Vec::new();
    while let Some((i, ch)) = it.next() {
        if ch != '"' && ch != '\'' {
            continue;
        }
        let close = body[i + 1..].find(ch)?;
        cur.push(body[i + 1..i + 1 + close].to_string());
        // 閉じの引用符まで読み飛ばす(**+1 を忘れると閉じが次の開きになる**)
        while it.peek().is_some_and(|(j, _)| *j <= i + close + 1) {
            it.next();
        }
        if cur.len() == 2 {
            let v = cur.pop().unwrap();
            out.push((cur.pop().unwrap(), v));
        }
    }
    Some(out)
}

/// plugins にある .py の名前(モジュール名)を並べる。
pub fn plugin_modules() -> Vec<String> {
    modules_in(&plugins_dir())
}

/// 置き場の .py の名前(モジュール名)を並べる。**置き場を選べる形**に
/// したのは、マクロ(plugins)と関数(funcs)で同じ走査を使うため
pub fn modules_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "py"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .collect();
    v.sort();
    v
}

/// plugins の .py の見出し — (モジュール名, その中の def の名前)。
/// **読むだけで実行しない**(@list の一覧に使う)。
pub fn plugin_outline() -> Vec<(String, Vec<String>)> {
    outline_in(&plugins_dir())
}

/// 置き場の .py の見出し — (モジュール名, その中の def の名前)
pub fn outline_in(dir: &std::path::Path) -> Vec<(String, Vec<String>)> {
    modules_in(dir)
        .into_iter()
        .map(|m| {
            let src = std::fs::read_to_string(dir.join(format!("{m}.py"))).unwrap_or_default();
            (m, def_names(&src))
        })
        .collect()
}

/// .py の中の `def 名前(` を並べる(先頭の桁のものだけ = 入れ子の def は数えない)。
pub fn def_names(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.strip_prefix("def "))
        .filter_map(|r| r.split_once('(').map(|(n, _)| n.trim().to_string()))
        .filter(|n| !n.starts_with('_'))
        .collect()
}

/// いまの plugins の姿(名前, 大きさ, 最終更新)。**置き場の時刻だけでは
/// 足りない** — 中の .py を書き換えても置き場の時刻は動かないので(項目の
/// 出入りでしか動かない)、1つ1つの名前・大きさ・時刻を見る。
pub fn plugins_shape() -> Vec<(String, u64, std::time::SystemTime)> {
    shape_in(&plugins_dir())
}

/// 置き場の姿(名前・大きさ・時刻)。**中を書き換えても置き場の時刻は
/// 動かない**ので、1つ1つを見る
pub fn shape_in(dir: &std::path::Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut v: Vec<_> = modules_in(dir)
        .into_iter()
        .filter_map(|m| {
            let md = std::fs::metadata(dir.join(format!("{m}.py"))).ok()?;
            Some((m, md.len(), md.modified().ok()?))
        })
        .collect();
    v.sort();
    v
}

// ---- サンドボックス(Cage)---------------------------------------------------

/// サンドボックスの種類。**Flatpak の中では bwrap の入れ子が動かない**(ユーザー名前空間の
/// 入れ子が塞がれている)ので、そこでは公式の入れ子口 flatpak-spawn --sandbox
/// を使う。どちらも組めなければ None — 他所から来たかもしれないコードは
/// サンドボックスの外では実行しない(そう言う)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cage {
    /// 素の Linux: /usr/bin/bwrap
    Bwrap,
    /// Flatpak の中: flatpak-spawn --sandbox(実機での実証はまだ —
    /// packaging/flatpak/README.md の実証項目を参照)
    Flatpak,
    /// サンドボックスが組めない
    None,
}

/// いまの環境で組めるサンドボックス。Flatpak の中かは /.flatpak-info で見分ける(公式の印)
pub fn cage_kind() -> Cage {
    if std::path::Path::new("/.flatpak-info").exists() {
        // flatpak-spawn は flatpak-xdg-utils の道具。Flatpak の runtime には居る
        if std::path::Path::new("/usr/bin/flatpak-spawn").exists() {
            Cage::Flatpak
        } else {
            Cage::None
        }
    } else if std::path::Path::new("/usr/bin/bwrap").exists() {
        Cage::Bwrap
    } else {
        Cage::None
    }
}

/// サンドボックスつき実行の作業場(交換用の読み書き領域)。Flatpak の --sandbox-expose は
/// **~/.var/app/$ID/sandbox の下しか見せられない**ので、置き場がサンドボックスで変わる
pub fn cage_work_dir(tag: &str) -> PathBuf {
    match cage_kind() {
        Cage::Flatpak => {
            // XDG_DATA_HOME = ~/.var/app/$ID/data(Flatpak が設定する)。
            // その親の sandbox/ が flatpak-spawn に見せられる場所
            let app = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .and_then(|d| d.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(std::env::temp_dir);
            app.join("sandbox").join(format!("{tag}-{}", std::process::id()))
        }
        _ => std::env::temp_dir().join(format!("{tag}-{}", std::process::id())),
    }
}

/// サンドボックスの中で Python を回す Command を組む。dir = cage_work_dir の作業場。
/// ro_binds = 読み取り専用で見せたい場所(.venv や .so の隣 — bwrap だけが使う。
/// Flatpak では /app と runtime が最初から見えている)。None = サンドボックスが組めない
pub fn caged_python(
    py: &std::path::Path,
    dir: &std::path::Path,
    ro_binds: &[PathBuf],
    allow_net: bool,
) -> Option<std::process::Command> {
    caged_python_with(cage_kind(), py, dir, ro_binds, allow_net)
}

/// サンドボックスの種類を外から差せる形(試験が引数の組みを確かめる)
pub fn caged_python_with(
    kind: Cage,
    py: &std::path::Path,
    dir: &std::path::Path,
    ro_binds: &[PathBuf],
    allow_net: bool,
) -> Option<std::process::Command> {
    match kind {
        Cage::Bwrap => {
            // サンドボックス: / は読み取り専用、ホームは空、書けるのは作業場だけ
            let mut c = std::process::Command::new("/usr/bin/bwrap");
            c.args(["--ro-bind", "/", "/", "--tmpfs", "/home", "--tmpfs", "/tmp"]);
            for p in ro_binds {
                if p.exists() {
                    c.arg("--ro-bind").arg(p).arg(p);
                }
            }
            c.arg("--bind").arg(dir).arg(dir);
            if !allow_net {
                c.arg("--unshare-net");
            }
            c.args([
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--die-with-parent",
                "--new-session",
                "--setenv",
                "HOME",
                "/tmp",
                "--",
            ]);
            c.arg(py);
            Some(c)
        }
        Cage::Flatpak => {
            // 公式の入れ子: バスは切れ、ファイルは /app と runtime と
            // expose した作業場だけ。網は --no-network で閉じる
            let name = dir.file_name()?.to_string_lossy().to_string();
            let mut c = std::process::Command::new("flatpak-spawn");
            c.arg("--sandbox");
            c.arg(format!("--sandbox-expose={name}"));
            if !allow_net {
                c.arg("--no-network");
            }
            c.arg(py);
            Some(c)
        }
        Cage::None => None,
    }
}

// ---- 実行 -------------------------------------------------------------------

/// 実行が届かなかった理由。**文言はここでは作らない** — 画面の言葉(訳)は
/// アプリの領分なので、アプリが写す(calc/src/py.rs の包みがその場所)
#[derive(Debug)]
pub enum RunErr {
    /// 起動できない(python が無い等)
    Spawn(std::io::Error),
    /// 時間切れ(秒数)— 殺してある
    Timeout(u64),
    /// 待ち合わせの失敗
    Wait(String),
}

/// 子プロセスを時間制限つきで回す → (成功か, stdout, stderr)。
/// 出力は別スレッドで吸い出す(パイプが詰まっても try_wait が止まらない)。
/// 時間切れは殺して Err — サンドボックスの中の無限ループでアプリの手が塞がらない
pub fn run_with_timeout(
    cmd: &mut std::process::Command,
    secs: u64,
) -> Result<(bool, String, String), RunErr> {
    use std::io::Read;
    use std::process::Stdio;
    // 証明書の道を渡す(py_env)。**ここが全部の実行の通り道** — 同梱の
    // Python は組んだ機械の径路を焼き付けていて、そのままだと https が
    // 全部落ちる(2026-08-14)。囲いの中でも /etc は読み取り専用で見える
    for (k, v) in py_env() {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(RunErr::Spawn)?;
    let mut so = child.stdout.take();
    let mut se = child.stderr.take();
    let th_o = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(r) = so.as_mut() {
            let _ = r.read_to_string(&mut s);
        }
        s
    });
    let th_e = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(r) = se.as_mut() {
            let _ = r.read_to_string(&mut s);
        }
        s
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let out = th_o.join().unwrap_or_default();
                let err = th_e.join().unwrap_or_default();
                return Ok((st.success(), out, err));
            }
            Ok(None) => {
                if start.elapsed().as_secs() >= secs {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = th_o.join();
                    let _ = th_e.join();
                    return Err(RunErr::Timeout(secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(RunErr::Wait(e.to_string())),
        }
    }
}

/// **venv の素にする Python。** 機械に入っている物を使います。
///
/// `find_python` と違って**利用者の venv は見ない** — 自分自身を素にして
/// 作り直すと、入れ子になって元の Python が分からなくなる。
///
/// *同梱の Python は見ません*(2026-08-24 に同梱をやめました)。
fn base_python() -> PathBuf {
    if let Some(p) = std::env::var_os("JO_PYTHON") {
        return p.into();
    }
    "python3".into()
}


/// 機械の証明書の束を探す(見つからなければ None)。
///
/// **配る Python は自分の径路を焼き付けている** — 同梱した python は
/// 「組んだ機械の /install/ssl/cert.pem」を見に行き、配った先には無いので
/// **https が全部落ちる**(2026-08-14 に見本の天気予報で踏んだ。この機械の
/// venv も旧名の径路を指したまま壊れていた)。だから走らせる側が
/// `SSL_CERT_FILE` で機械の束を教える。置き場は配り物ごとに違うので、
/// よくある順に探す
pub fn ca_bundle() -> Option<PathBuf> {
    const CANDS: &[&str] = &[
        "/etc/ssl/certs/ca-certificates.crt",       // Debian/Ubuntu
        "/etc/pki/tls/certs/ca-bundle.crt",         // Fedora/RHEL
        "/etc/ssl/ca-bundle.pem",                   // openSUSE
        "/etc/ssl/cert.pem",                        // Alpine/macOS(Homebrew)
        "/usr/local/etc/openssl/cert.pem",          // macOS(Homebrew の別置き)
    ];
    CANDS.iter().map(PathBuf::from).find(|p| p.exists())
}

/// 子の Python に渡す環境。いまは証明書の道だけ。
/// **既に指されていれば触らない**(利用者の設定が勝つ)
pub fn py_env() -> Vec<(&'static str, String)> {
    if std::env::var_os("SSL_CERT_FILE").is_some() {
        return Vec::new();
    }
    ca_bundle()
        .map(|p| vec![("SSL_CERT_FILE", p.display().to_string())])
        .unwrap_or_default()
}

/// **いま開いている綴り(フォルダ)。** アプリがフォルダを開いたときに置きます。
///
/// エディタと同じで、*作業しているフォルダの `.venv` を最優先*にするためです
/// (2026-08-24 発注者「zed と同じように作業ディレクトリー内の仮想環境を
/// 優先でいいでしょう」)。ここが空なら、今までどおりの順で探します。
static 綴り: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// いま開いている綴りを教える。アプリがフォルダを開いた回に呼びます。
pub fn set_work_dir(dir: Option<PathBuf>) {
    if let Ok(mut g) = 綴り.lock() {
        *g = dir;
    }
}

/// いま開いている綴り(試験と、案内の文言のため)。
pub fn work_dir() -> Option<PathBuf> {
    綴り.lock().ok().and_then(|g| g.clone())
}

/// 裏方の Python を探す。
/// **JO_PYTHON → 綴りの .venv → 開発機の .venv → 利用者の venv → python3**。
/// matplotlib が居るかは実行して分かる(居なければ status で言う)。
///
/// # 同梱の Python は見ません(2026-08-24 発注者)
///
/// 前は最後から2番目に「配る形に同梱した Python」を見ていました。
/// *同梱をやめたので、この段も外しました。* 同梱の Python は読むだけの物で、
/// matplotlib も polars も入っていません — 結局その2つは利用者が
/// `.venv` に入れる必要があり、*同梱があってもなくても手順は同じ*でした。
pub fn find_python() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("JO_PYTHON") {
        return p.into();
    }
    // **開いている綴りの `.venv` がいちばん強い。** エディタと同じ作法です。
    // 同じフォルダを JupyterLab とエディタと officework が見ているとき、
    // *3つとも同じ Python を使う*のが期待される動きです
    if let Some(d) = work_dir() {
        let p = d.join(".venv/bin/python");
        if p.exists() {
            return p;
        }
        let w = d.join(".venv/Scripts/python.exe"); // Windows の venv
        if w.exists() {
            return w;
        }
    }
    // **開発機を先に見る。** 実行ファイルを遡って `.venv` が見つかるのは
    // 「リポジトリの中から走らせている」印で、そのときは repo の `.venv`
    // (polars 等が入っている物)が正しい。配った物の実行ファイルは
    // リポジトリの中に居ないので、ここは素通りする。
    //
    // **この順を間違えて一度踏んだ**(2026-08-17): 利用者の venv を先に
    // 見る形にしたら、開発機で出来たての空の venv が repo の `.venv` に
    // 勝ち、polars が見えなくなった
    let venv = std::path::Path::new(".venv/bin/python");
    if venv.exists() {
        return venv.into();
    }
    // 実行ファイルの場所から遡って探す(target/release/calc → リポジトリ直下)。
    // **どこから起動しても同じ python に当たる** — CWD 頼みだと
    // 「polars がありません」になり、ピボットが置けない
    // (発注者の実機で踏んだ 2026-08-07)
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1) {
            let p = dir.join(".venv/bin/python");
            if p.exists() {
                return p;
            }
        }
    }
    // **利用者が足した物がここに載る**(~/.config/officework/.venv)。
    // 同梱より先に見るのがこの順の要 — 同梱は読むだけの物で、足した物は
    // こちらに居る。`--system-site-packages` なので同梱の中身も見える。
    // **壊れていたら飛ばす**(次の同梱に落ちる)— 直すのは ensure_venv の仕事
    if let Some(p) = venv_python() {
        if !venv_broken() {
            return p;
        }
    }
    "python3".into()
}

// ---- 裏方の台本(呼ぶ側がデータを JSON 等で渡す)----------------------------
//
// **これらは「指図 → 答え」の純関数**(2026-08-16 に契約を書き留めた)。
// 呼ぶ側(Rust)が選択範囲を読んで JSON に組み、台本はそれだけを見て
// 答え(画像・CSV・数)を返す。**ブックには触れない** — RPC の口も
// `officework` の取り込みも持たない。
//
// 利用者がリボンに足すマクロ([`ribbon_dir`])は**別の契約**で、押されたら
// RPC で動いているブックを操る。揃えようとすると、こちらの台本に
// 今は無いブックへの手を与えることになる — 見た目のために安全を下げる
// 取引なので、揃えない。**2つの契約があることを書いておく方を選んだ。**

/// グラフの台本(matplotlib)。データは JSON で渡す。
/// 日本語は機械のフォントを matplotlib に登録して出す(豆腐にしない)。
pub const CHART_PY: &str = r#"
import json, sys
import matplotlib
matplotlib.use("Agg")
import numpy as np
from matplotlib import font_manager, pyplot as plt

spec = json.load(open(sys.argv[1], encoding="utf-8"))
if spec.get("font"):
    try:
        font_manager.fontManager.addfont(spec["font"])
        plt.rcParams["font.family"] = font_manager.FontProperties(
            fname=spec["font"]).get_name()
    except Exception:
        pass
labels = spec["labels"]
x = np.arange(len(labels))
series = spec["series"] or [{"name": "", "values": [0] * len(labels)}]
n = len(series)
w = 0.8 / n
fig, ax = plt.subplots(figsize=(6.4, 4.0))
# 種類は指図から。無ければ棒(これまでどおり)
kind = spec.get("kind", "bar")
for i, s in enumerate(series):
    if kind == "line":
        # **空の所は線を切る。**予測シートは実績と予測を別の列に置くので、
        # 0 として繋ぐと谷ができる
        ys = [float("nan") if v is None else v for v in s["values"]]
        ax.plot(x, ys, marker="", label=s["name"])
    else:
        ax.bar(x + (i - (n - 1) / 2) * w, s["values"], w, label=s["name"])
ax.set_xticks(x)
ax.set_xticklabels(labels)
# 目盛りが多いときは間引いて傾ける。全部まっすぐ出すと重なって読めない
if len(labels) > 10:
    step = (len(labels) + 9) // 10
    for k, t in enumerate(ax.get_xticklabels()):
        t.set_visible(k % step == 0)
    for t in ax.get_xticklabels():
        t.set_rotation(45)
        t.set_ha("right")
if n > 1:
    ax.legend()
fig.tight_layout()
fig.savefig(spec["out"], dpi=100)
"#;

pub const CSV_PY: &str = r#"
import csv, sys

# argv: path [文字コード|auto] [区切り|auto]
path = sys.argv[1]
want_enc = sys.argv[2] if len(sys.argv) > 2 else "auto"
want_delim = sys.argv[3] if len(sys.argv) > 3 else "auto"
raw = open(path, "rb").read()
text = None
used_enc = ""
encs = ("utf-8-sig", "cp932", "latin-1") if want_enc == "auto" else (want_enc,)
for enc in encs:
    try:
        text = raw.decode(enc)
        used_enc = enc
        break
    except UnicodeDecodeError:
        continue
if text is None:
    sys.exit("その文字コードでは読めません" if want_enc != "auto" else "文字コードが判定できません")
if want_delim == "auto":
    try:
        dialect = csv.Sniffer().sniff(text[:4096], delimiters=",\t;")
    except csv.Error:
        dialect = csv.excel_tab if "\t" in text[:4096] else csv.excel
    used_delim = dialect.delimiter
    rows = list(csv.reader(text.splitlines(), dialect))
else:
    used_delim = want_delim
    rows = list(csv.reader(text.splitlines(), delimiter=want_delim))
# 1行目は下ごしらえの報告(使った文字コード・区切り)
meta = "\x01" + "\x1f".join((used_enc, used_delim))
out = meta + "\x1e" + "\x1e".join("\x1f".join(row) for row in rows)
sys.stdout.buffer.write(out.encode("utf-8"))
"#;

/// 方程式の台本(matplotlib の mathtext)。式を清書して透過 PNG に描く。
pub const EQ_PY: &str = r#"
import json, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib import font_manager

spec = json.load(open(sys.argv[1], encoding="utf-8"))
if spec.get("font"):
    try:
        font_manager.fontManager.addfont(spec["font"])
        plt.rcParams["font.family"] = font_manager.FontProperties(
            fname=spec["font"]).get_name()
    except Exception:
        pass
fig = plt.figure()
t = fig.text(0.05, 0.5, "$%s$" % spec["tex"], fontsize=20)
fig.canvas.draw()  # 式が読めなければここで止まる(黙って白紙にしない)
bbox = t.get_window_extent()
fig.set_size_inches(bbox.width / fig.dpi + 0.15, bbox.height / fig.dpi + 0.15)
plt.savefig(spec["out"], dpi=200, transparent=True)
"#;

/// テキストアートの台本(matplotlib)。太字+塗り+縁取りの飾り文字を
/// 透過 PNG に描く(色は calc の緑)。
pub const TEXTART_PY: &str = r##"
import json, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.patheffects as pe
from matplotlib import font_manager

spec = json.load(open(sys.argv[1], encoding="utf-8"))
if spec.get("font"):
    try:
        font_manager.fontManager.addfont(spec["font"])
        plt.rcParams["font.family"] = font_manager.FontProperties(
            fname=spec["font"]).get_name()
    except Exception:
        pass
fig = plt.figure()
t = fig.text(0.05, 0.5, spec["tex"], fontsize=44, fontweight="bold",
             color="#1B6E3C",
             path_effects=[pe.withStroke(linewidth=6, foreground="#D5E8DC")])
fig.canvas.draw()
bbox = t.get_window_extent()
fig.set_size_inches(bbox.width / fig.dpi + 0.2, bbox.height / fig.dpi + 0.2)
plt.savefig(spec["out"], dpi=200, transparent=True)
"##;

/// ソルバーの台本(scipy)。指図は JSON、答えは \x1f 区切りの変数の値。
pub const SOLVER_PY: &str = r#"
import json, sys
from scipy.optimize import linprog

spec = json.load(open(sys.argv[1], encoding="utf-8"))
n = len(spec["c"])
lo = 0 if spec["nonneg"] else None
# 枠は変数ごと。無ければ従来どおり(非負なら 0 から)
bounds = [tuple(b) for b in spec.get("bounds") or []] or [(lo, None)] * n
# 整数の印(0=普通 1=整数)。全部 0 なら渡さない — 素の LP のままにする
integrality = spec.get("integrality") or []
kw = {}
if any(integrality):
    # **HiGHS の分枝限定**(scipy 1.9 以降)。バイナリは 0〜1 の枠つきの整数
    kw["integrality"] = integrality
r = linprog(
    c=spec["c"],
    A_ub=spec["aub"] or None,
    b_ub=spec["bub"] or None,
    A_eq=spec["aeq"] or None,
    b_eq=spec["beq"] or None,
    bounds=bounds,
    method="highs",
    **kw,
)
if not r.success:
    sys.exit("解がありません: " + str(r.message))
# **整数の答えは丸めて返す。** 分枝限定は 3.0000000001 のような値を返す
# ことがあり、そのままセルに置くと「整数にしたのに整数でない」に見えます
out = [round(v) if i < len(integrality) and integrality[i] else v
       for i, v in enumerate(r.x)]
sys.stdout.write("\x1f".join("%.12g" % v for v in out))
"#;

/// ピボットの台本(polars)。指図は JSON、答えは CSV 取り込みと同じ
/// 区切りの印(\x1e 行 / \x1f 欄)で返す。
/// **予測シートの中身**(2026-08-22。台帳の [大])。
///
/// 加法の Holt-Winters(指数平滑)です。水準・傾き・季節の3つを、
/// 1期先の誤差で少しずつ直しながら進みます。α・β・γ は当てはまりが
/// いちばん良くなる所を scipy が探します。
///
/// **季節の長さは自動で選びます。**ただし2つの条件を付けました。
///
/// * 3周期ぶん無い長さは試さない(2周期では、たまたま似た形が2度出ただけの
///   ものを季節と呼んでしまいます)
/// * 季節の振れが雑音の2倍に届かないなら季節としない(ただの上り坂+雑音に
///   季節 3 を見つけたので足しました)
///
/// 区間は ETS(A,A,A) の分散の式から出します。**見込みであって約束では
/// ありません** — 呼ぶ側はそう言って出します。
pub const FORECAST_PY: &str = r#"
import json, sys
import numpy as np
from scipy.optimize import minimize
from scipy.stats import norm

spec = json.load(open(sys.argv[1], encoding="utf-8"))
y = np.asarray(spec["values"], dtype=float)
h = int(spec.get("horizon", 6))
conf = float(spec.get("conf", 0.95))
want_m = int(spec.get("season", 0))
n = len(y)

def 組む(y, m, a, b, g):
    """加法の Holt-Winters。返しは (1期先の予測の列, 最後の水準, 傾き, 季節)"""
    if m > 1:
        mu = float(y[:m].mean())
        tr = (float(y[m:2 * m].mean()) - mu) / m if n >= 2 * m else 0.0
        # **季節の初期値から傾きを抜く。**抜かないと、1周期の中の上り坂まで
        # 季節だと思い込み、当てはまりが悪くなる(見本の SSE が 2744 → 0.0)
        sea = [float(y[i] - (mu + (i - (m - 1) / 2) * tr)) for i in range(m)]
        lvl = mu - ((m - 1) / 2 + 1) * tr
    else:
        lvl, tr, sea = y[0], (y[1] - y[0]) if n > 1 else 0.0, [0.0]
    fit = []
    for t in range(n):
        s = sea[t % m] if m > 1 else 0.0
        f = lvl + tr + s
        fit.append(f)
        e = y[t] - f
        lvl2 = lvl + tr + a * e
        tr = tr + a * b * e
        if m > 1:
            sea[t % m] = s + g * e
        lvl = lvl2
    return np.asarray(fit), lvl, tr, sea

def sse(p, y, m):
    a, b, g = p
    fit, _, _, _ = 組む(y, m, a, b, g)
    r = y - fit
    v = float(np.sum(r * r))
    return v if np.isfinite(v) else 1e18

def 合わせる(y, m):
    best = None
    for start in ((0.3, 0.1, 0.1), (0.6, 0.05, 0.3), (0.05, 0.01, 0.5)):
        r = minimize(sse, start, args=(y, m), method="L-BFGS-B",
                     bounds=[(1e-4, 0.999)] * 3)
        if best is None or r.fun < best.fun:
            best = r
    a, b, g = best.x
    fit, lvl, tr, sea = 組む(y, m, a, b, g)
    k = 3 + (m if m > 1 else 1)
    s2 = float(best.fun) / max(n - k, 1)
    # AICc。季節を増やすほど当てはまるので、罰を付けて選ぶ
    ll = -0.5 * n * (np.log(2 * np.pi * max(s2, 1e-12)) + 1)
    aic = -2 * ll + 2 * k
    aicc = aic + (2 * k * (k + 1) / (n - k - 1)) if n - k - 1 > 0 else aic + 1e6
    return dict(m=m, a=a, b=b, g=g, fit=fit, lvl=lvl, tr=tr, sea=sea, s2=s2, aicc=aicc)

# 季節の長さを選ぶ。**2周期ぶん無ければその長さは試さない**
# **3周期ぶん無い長さは試さない。**2周期では、たまたま似た形が2度出ただけの
# ものを季節と呼んでしまう
候補 = [1]
if want_m > 1:
    候補 = [want_m] if n >= 2 * want_m else [1]
elif want_m == 0:
    候補 += [m for m in range(2, min(n // 3, 24) + 1)]
結果 = [合わせる(y, m) for m in 候補]
無季節 = 結果[0]
季節あり = [r for r in 結果[1:]]
best = 無季節
if 季節あり:
    c = min(季節あり, key=lambda r: r["aicc"])
    # **はっきり良いときだけ季節を採る。**AICc が 2 以上良ければ「はっきり」
    # (統計の慣例)。僅差なら季節なしに倒す — 無い季節を見つける方が害が大きい
    # **季節の振れが雑音より大きいこと**も条件にする。AICc だけだと、
    # ただの上り坂+雑音に季節 3 を見つけてしまう(実際に踏んだ)。
    # 振れ幅が残差の2倍に届かないなら、それは季節ではなく雑音
    振れ = max(c["sea"]) - min(c["sea"])
    if c["aicc"] < 無季節["aicc"] - 2.0 and 振れ >= 2 * np.sqrt(c["s2"]):
        best = c
m, a, b, g = best["m"], best["a"], best["b"], best["g"]
lvl, tr, sea = best["lvl"], best["tr"], best["sea"]

fc, lo, up = [], [], []
z = float(norm.ppf(0.5 + conf / 2))
cum = 0.0
for j in range(1, h + 1):
    s = sea[(n + j - 1) % m] if m > 1 else 0.0
    v = lvl + j * tr + s
    fc.append(float(v))
    if j > 1:
        c = a * (1 + (j - 1) * b) + (g if (m > 1 and (j - 1) % m == 0) else 0.0)
        cum += c * c
    sd = np.sqrt(best["s2"] * (1 + cum))
    lo.append(float(v - z * sd))
    up.append(float(v + z * sd))

print(json.dumps({
    "ok": True, "season": m, "alpha": a, "beta": b, "gamma": g,
    "sigma": float(np.sqrt(best["s2"])),
    "fit": [float(x) for x in best["fit"]],
    "forecast": fc, "lower": lo, "upper": up,
}, ensure_ascii=False, separators=(",", ":")))
"#;

/// **PDF の表を取り出す。**
///
/// PDF に「表」という構造はありません。あるのは字と、字の置かれた座標と、
/// 線だけです。だから表は**推し量る**ことになります。外すと、数字が黙って
/// 隣の桁へずれます。そこでこの台本は、
///
/// * まず**罫線で**切ります(`lines`)。線が引いてある表はこれで正確に取れます
/// * 線が無ければ**文字の位置で**切ります(`text`)
/// * **どちらで取ったかを必ず返します**(`lines` か `text`)。呼ぶ側は
///   それを訳して画面に出します。台本は鍵だけを返し、画面の字は作りません
///
/// 返すのは JSON で、ページごと・表ごとの升目です。**この台本は何も書き
/// 込みません** — 人が見て、押してから流し込みます。
pub const PDF_TABLE_PY: &str = r#"
import sys
import pdfplumber

path = sys.argv[1]
out = []
with pdfplumber.open(path) as pdf:
    for pno, page in enumerate(pdf.pages, start=1):
        # 罫線で切る。線の引いてある表はこれが正確
        found = []
        for how, settings in (
            ("lines", {"vertical_strategy": "lines", "horizontal_strategy": "lines"}),
            ("text", {"vertical_strategy": "text", "horizontal_strategy": "text"}),
        ):
            try:
                tables = page.extract_tables(settings)
            except Exception:
                tables = []
            # 1行1列しかないものは表ではない(ただの段落)
            tables = [t for t in tables if t and len(t) > 1 and max(len(r) for r in t) > 1]
            if tables:
                found = [(how, t) for t in tables]
                break
        for how, t in found:
            grid = [["" if c is None else str(c).replace("\n", " ").strip() for c in row]
                    for row in t]
            # 空だけの行は落とす。文字の位置で切ると、行の隙間が1行として
            # 出てくることがある(実物で見た)。**中身のある行は落としません**
            grid = [r for r in grid if any(c for c in r)]
            if len(grid) < 2:
                continue
            w = max(len(r) for r in grid)
            for r in grid:
                r.extend([""] * (w - len(r)))
            # 右端の空だけの列も落とす(同じ理由)
            while w > 1 and all(r[w - 1] == "" for r in grid):
                for r in grid:
                    r.pop()
                w -= 1
            out.append({"page": pno, "how": how, "rows": grid})

# 返しは区切り文字づけ(この形はこちらで決めたもの。JSON は要らない)
#   表と表 = \x1d / 見出しと中身 = \x1e / 行と行 = \x1e / 升と升 = \x1f
#   各表の頭は  ページ番号 \x1f 取り方(lines / text)
parts = []
for t in out:
    head = str(t["page"]) + "\x1f" + t["how"]
    body = "\x1e".join("\x1f".join(r) for r in t["rows"])
    parts.append(head + "\x1e" + body)
sys.stdout.write("\x1d".join(parts))
"#;

pub const PIVOT_PY: &str = r#"
import json, sys
import polars as pl

spec = json.load(open(sys.argv[1], encoding="utf-8"))
headers = spec["headers"]
data = {h: [row[i] for row in spec["rows"]] for i, h in enumerate(headers)}
df = pl.DataFrame(data)
# 絞り込み(見出しの ▼)。隠す値を先に落としてから集計する
for _f, _vs in spec.get("hide", []):
    if _f in df.columns and _vs:
        df = df.filter(~pl.col(_f).is_in(_vs))

# グループ化(第2版)。日付は 月/四半期/年 の札に、数は幅Nの帯に置き換える
def _grouped(col, unit):
    if unit.startswith("幅:"):
        w = float(unit[2:])
        f = pl.col(col).cast(pl.Float64, strict=False)
        lo = (f / w).floor() * w
        def _n(x):
            return str(int(x)) if float(x).is_integer() else str(x)
        # 帯の札は数字の幅で右詰め — 文字順でも 0〜49 < 50〜99 < 100〜149 に並ぶ
        _mx = df[col].cast(pl.Float64, strict=False).max()
        _wid = len(_n((_mx // w) * w + w - (1 if w.is_integer() else 0))) if _mx is not None else 0
        return (pl.when(f.is_null()).then(pl.col(col))
                .otherwise(lo.map_elements(
                    lambda v: (_n(v).rjust(_wid) + "〜" +
                               _n(v + w - (1 if w.is_integer() else 0)).rjust(_wid)),
                    return_dtype=pl.String)))
    d = pl.col(col).str.strptime(pl.Date, "%Y-%m-%d", strict=False)
    d2 = pl.col(col).str.strptime(pl.Date, "%Y/%m/%d", strict=False)
    d = pl.coalesce(d, d2)
    if unit == "years":
        out = d.dt.strftime("%Y年")
    elif unit == "quarters":
        out = (d.dt.year().cast(pl.String) + "年Q" +
               ((d.dt.month() + 2) // 3).cast(pl.String))
    else:  # months
        out = d.dt.strftime("%Y-%m")
    # 日付として読めない値はそのまま残す(黙って落とさない)
    return pl.when(d.is_null()).then(pl.col(col)).otherwise(out)

for _f, _u in spec.get("group", []):
    if _f in df.columns:
        df = df.with_columns(_grouped(_f, _u).alias(_f))
val, agg = spec["value"], spec["agg"]
# **画面に出る札は Rust が訳して渡します**(2026-08-26)。台本は鍵で
# 処理し、字は訳で書きます — ここから対訳表は引けません
AGG_LABEL = spec.get("agg_label", agg)
SUB_LABEL = spec.get("subtotal_label", "{} subtotal")
GRAND_LABEL = spec.get("grand_label", "Grand totals")
if agg != "count":
    # 数にならないものは null(集計から外れる)
    df = df.with_columns(pl.col(val).cast(pl.Float64, strict=False))
idx, cols = spec["index"], spec["columns"]
# **粒と集計の名前は英語で受けます**(2026-08-26 の移行)。Rust 側が
# 品書きの鍵をそのまま渡すので、鍵が英語になったらここも英語です
FN = {"sum": "sum", "average": "mean", "count": "len",
      "maximum": "max", "minimum": "min"}

def agg_expr():
    return {"sum": pl.sum(val), "average": pl.mean(val),
            "count": pl.len().alias(val),
            "maximum": pl.max(val), "minimum": pl.min(val)}[agg]

def table(frame, index):
    if cols:
        return frame.pivot(cols, index=index, values=val,
                           aggregate_function=FN[agg], sort_columns=True).sort(index)
    return frame.group_by(index).agg(agg_expr()).sort(index)

def stub(frame, label, index):
    # index の1列目に札を立て、残りを空にした複製。ピボットに通すことで
    # 列名の並びを main と揃えたまま「1行に集めた」答えが得られる
    ex = [pl.lit(label).alias(index[0])] + [pl.lit("").alias(i) for i in index[1:]]
    return frame.with_columns(ex)

def row_total(frame, index):
    # 行ごとの総計(列に広げたぶんを全部まとめた値)。集計の種類を守る
    return {tuple(r[:-1]): r[-1]
            for r in frame.group_by(index).agg(agg_expr().alias("_t")).rows()}

main = table(df, idx)
# 値のフィルター(第2版)。集計した後の行に掛ける — 列に広げていれば
# 行の総計、そうでなければ値の列そのもの
vf = spec.get("vfilter")
if vf:
    _op, _th = vf
    _OPS = {">": lambda x: x > _th, ">=": lambda x: x >= _th,
            "<": lambda x: x < _th, "<=": lambda x: x <= _th,
            "=": lambda x: x == _th}
    _keys = row_total(df, idx) if cols else None
    def _row_ok(r):
        x = _keys.get(tuple(r[:len(idx)])) if _keys is not None else r[-1]
        try:
            return x is not None and _OPS[_op](float(x))
        except (TypeError, ValueError, KeyError):
            return False
    main = pl.DataFrame([list(r) for r in main.rows() if _row_ok(r)],
                        schema=main.schema, orient="row") if main.height else main
    # フィルター後の物差しで小計・総計も出す — 見えない行を数えない
    df = df.filter(
        pl.concat_str([pl.col(i).cast(pl.String) for i in idx], separator="\x1f")
        .is_in([chr(31).join(str(v) for v in r[:len(idx)]) for r in main.rows()])
        if main.height else pl.lit(False))
tot_col = spec["totals"] and bool(cols)
tots = row_total(df, idx) if tot_col else {}

out = []  # (種別, 欄) 種別: d=データ s=小計 b=空行 t=総計

sub = None
if spec["subtotals"] and len(idx) >= 2:
    sub = {r[0]: list(r[1:]) for r in table(df, [idx[0]]).rows()}
    sub_tots = row_total(df, [idx[0]]) if tot_col else {}

# 1つ目の見出しで束ねながら吐く(小計・空行はその区切りごと)
groups = []
for r in main.rows():
    if groups and groups[-1][0] == r[0]:
        groups[-1][1].append(r)
    else:
        groups.append((r[0], [r]))

for g, rs in groups:
    prev = None
    for r in rs:
        cells = list(r)
        if spec["compact"] and prev is not None:
            # 繰り返しの見出しを空欄に(コンパクト形式)
            for i in range(len(idx)):
                if cells[i] == prev[i]:
                    cells[i] = ""
                else:
                    break
        if tot_col:
            cells.append(tots.get(tuple(r[:len(idx)])))
        out.append(("d", cells))
        prev = list(r)
    if sub is not None:
        cells = [SUB_LABEL.replace("{}", str(g))] + [""] * (len(idx) - 1) + sub[g]
        if tot_col:
            cells.append(sub_tots.get((g,)))
        out.append(("s", cells))
    if spec["blank_rows"] and len(idx) >= 2:
        out.append(("b", [""] * (len(main.columns) + (1 if tot_col else 0))))

if spec["totals"] and df.height:
    cells = list(table(stub(df, GRAND_LABEL, idx), idx).rows()[0])
    if tot_col:
        cells.append(df.select(agg_expr()).item())
    out.append(("t", cells))

# 並べ替え(2026-08-13、台帳「ピボットの並べ替え」)。
# **小計・空行を出しているときは掛けない** — 「d」の塊の間に区切りが
# 挟まっており、並べ替えると区切りと中身の対応が崩れるため。黙って
# 崩さずに、掛けなかったことを答えに載せる
_so = spec.get("sort", "")
if _so:
    _n = len(idx)
    _pos = [k for k, (kind, _c) in enumerate(out) if kind == "d"]
    _block = bool(_pos) and (_pos[-1] - _pos[0] + 1) == len(_pos)
    # 断るのは呼ぶ側(calc)の役目。ここは念のための素通し
    if _block and _pos:
        _rows = [out[k][1] for k in _pos]
        def _label(c):
            return tuple("" if x is None else str(x) for x in c[:_n])
        def _val(c):
            v = c[_n] if len(c) > _n else None
            return v if isinstance(v, (int, float)) else float("-inf")
        if _so == "見出しの昇順":
            _rows.sort(key=_label)
        elif _so == "見出しの降順":
            _rows.sort(key=_label, reverse=True)
        elif _so == "値の大きい順":
            _rows.sort(key=_val, reverse=True)
        elif _so == "値の小さい順":
            _rows.sort(key=_val)
        for k, c in zip(_pos, _rows):
            out[k] = ("d", c)

# 計算の種類(比率・累計・差)。データ行の値の欄だけを置き換える。
# **累計と差は小計・総計を出さない**(積み上げの途中に総計が挟まると
# 読み違えるため)ので、呼ぶ側が totals/subtotals を落として渡す
_sa = spec.get("show_as", "")
if _sa:
    _n = len(idx)  # 見出しの欄数。ここから右が値
    _cells = [c for k, c in out if k == "d"]
    if _sa == "比率":
        # 総計(全データの集計)を 100% とする
        _g = df.select(agg_expr()).item() if df.height else None
        for c in _cells:
            for j in range(_n, len(c)):
                v = c[j]
                c[j] = (v / _g) if (isinstance(v, (int, float)) and _g) else None
    elif _sa in ("累計", "差"):
        _prev = [None] * (max((len(c) for c in _cells), default=0))
        for c in _cells:
            for j in range(_n, len(c)):
                v = c[j]
                p = _prev[j]
                if isinstance(v, (int, float)):
                    if _sa == "累計":
                        c[j] = v + (p if isinstance(p, (int, float)) else 0)
                        _prev[j] = c[j]
                    else:
                        c[j] = (v - p) if isinstance(p, (int, float)) else None
                        _prev[j] = v
                else:
                    c[j] = None

def s(v):
    if v is None:
        return ""
    if _sa == "比率" and isinstance(v, float):
        return "%.1f%%" % (v * 100.0)
    if isinstance(v, float):
        return "%g" % v
    return str(v)

head = list(main.columns) + ([GRAND_LABEL] if tot_col else [])
lines = []
if cols:
    # Excel と同じ1行目の札: 「合計 / 金額」と、列に広げた見出し(月)
    label = [f"{AGG_LABEL} / {val}"] + [""] * (len(idx) - 1) + [" / ".join(cols)]
    label += [""] * (len(head) - len(label))
    lines.append("l\x1f" + "\x1f".join(label))
else:
    # 列が無いときは値の列の見出しを「合計 / 金額」に(Excel と同じ)
    head[-2 if tot_col else -1] = f"{AGG_LABEL} / {val}"
lines.append("h\x1f" + "\x1f".join(head))
for kind, cells in out:
    lines.append(kind + "\x1f" + "\x1f".join(s(v) for v in cells))
sys.stdout.buffer.write("\x1e".join(lines).encode("utf-8"))
"#;

/// **同梱の台本の一覧**(名前, 中身)。読む道を作るために名前を付けた
/// — 何が入っているかを見せない物は、直せるかどうかも判断できない。
/// 走らせる道はここからは開かない(呼ぶ側が指図を組んで初めて動く)
pub const BUNDLED: &[(&str, &str)] = &[
    ("chart", CHART_PY),
    ("pivot", PIVOT_PY),
    ("pdf_table", PDF_TABLE_PY),
    ("forecast", FORECAST_PY),
    ("solver", SOLVER_PY),
    ("csv", CSV_PY),
    ("equation", EQ_PY),
    ("textart", TEXTART_PY),
];

#[cfg(test)]
mod venv_tests {
    use super::*;

    /// 置き場は**マクロと同じフォルダの中**。エディタで開いたときに
    /// 隣に居ることがこの決めの理由(発注者 2026-08-17)
    #[test]
    fn venvはマクロと同じ置き場に居る() {
        assert_eq!(venv_dir().parent(), Some(config_dir().as_path()));
        assert_eq!(venv_dir().file_name().unwrap(), ".venv");
        // funcs と兄弟であること(エディタが1つのフォルダで両方見る)
        assert_eq!(funcs_dir().parent(), venv_dir().parent());
    }

    /// **焼き付いた素の Python が消えていたら壊れている。**
    /// venv は作った時の径路を pyvenv.cfg に持つので、アプリを入れ直して
    /// 径路が変わると動かない(2026-08-14 に一度踏んだ)
    #[test]
    fn 素のpythonが消えたvenvは壊れていると分かる() {
        let dir = std::env::temp_dir().join(format!("ow-venv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let 見る = |cfg: &str| -> bool {
            let home = cfg
                .lines()
                .find_map(|l| l.strip_prefix("home").map(|r| r.trim_start_matches([' ', '=']).trim()));
            match home {
                Some(h) => !std::path::Path::new(h).exists(),
                None => true,
            }
        };
        // 在る径路を指していれば壊れていない
        assert!(!見る(&format!("home = {}\n", dir.display())), "在る径路を壊れていると言った");
        // 消えた径路を指していれば壊れている
        assert!(見る("home = /nowhere/bin\n"), "消えた径路を見逃した");
        // home が無い pyvenv.cfg も壊れている扱い
        assert!(見る("version = 3.14.6\n"), "home の無い cfg を見逃した");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// pip の径路は**在らなくても綴りを返す**(案内に出すため)
    #[test]
    fn pipの径路は案内のために常に綴れる() {
        let p = venv_pip();
        assert!(p.starts_with(venv_dir()));
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("pip"), "pip ではない: {name}");
    }
}

#[cfg(test)]
mod cage_tests {
    use super::*;

    fn args_of(c: &std::process::Command) -> Vec<String> {
        std::iter::once(c.get_program().to_string_lossy().to_string())
            .chain(c.get_args().map(|a| a.to_string_lossy().to_string()))
            .collect()
    }

    #[test]
    fn bwrapのサンドボックスは網を既定で切る() {
        let d = PathBuf::from("/tmp/jo-py-1");
        let py = std::path::Path::new("python3");
        let c = caged_python_with(Cage::Bwrap, py, &d, &[], false).unwrap();
        let a = args_of(&c);
        assert_eq!(a[0], "/usr/bin/bwrap");
        assert!(a.contains(&"--unshare-net".into()), "網が切れていない: {a:?}");
        assert!(a.contains(&"--die-with-parent".into()));
        // net を許したときだけ網が通る
        let c2 = caged_python_with(Cage::Bwrap, py, &d, &[], true).unwrap();
        assert!(!args_of(&c2).contains(&"--unshare-net".into()));
    }

    #[test]
    fn flatpakのサンドボックスは公式の入れ子口を使う() {
        // Flatpak の中では bwrap の入れ子が動かない — flatpak-spawn --sandbox
        let d = PathBuf::from("/x/sandbox/jo-udf-9");
        let py = std::path::Path::new("python3");
        let c = caged_python_with(Cage::Flatpak, py, &d, &[], false).unwrap();
        let a = args_of(&c);
        assert_eq!(a[0], "flatpak-spawn");
        assert!(a.contains(&"--sandbox".into()), "{a:?}");
        assert!(
            a.contains(&"--sandbox-expose=jo-udf-9".into()),
            "作業場が expose されていない: {a:?}"
        );
        assert!(a.contains(&"--no-network".into()), "網が切れていない: {a:?}");
        assert!(!args_of(&caged_python_with(Cage::Flatpak, py, &d, &[], true).unwrap())
            .contains(&"--no-network".into()));
        // サンドボックスが組めなければ None(「実行しない」と言うのは呼ぶ側)
        assert!(caged_python_with(Cage::None, py, &d, &[], false).is_none());
    }

    #[test]
    fn defの名前は先頭の桁だけ数える() {
        let src = "def 集計(r):\n    def _中(x):\n        pass\ndef _隠し(x):\n    pass\ndef 倍(x): ...\n";
        assert_eq!(def_names(src), vec!["集計", "倍"]);
    }

    #[test]
    fn リボンの名乗りを走らせずに読む() {
        let src = "リボン = {\"札\": \"月次の締め\", \"絵\": \"py-list\"}\n\nprint(1)\n";
        let kv = decl_dict(src).unwrap();
        assert_eq!(kv, vec![("札".into(), "月次の締め".into()), ("絵".into(), "py-list".into())]);
        // 英語の綴りと ' の引用も受ける
        let en = "ribbon = {'label': 'Month end', 'tab': 'マクロ'}\n";
        assert_eq!(
            decl_dict(en).unwrap(),
            vec![("label".into(), "Month end".into()), ("tab".into(), "マクロ".into())]
        );
        // 名乗りが無ければ None(置き忘れをボタンにしない)
        assert!(decl_dict("def 走る():\n    pass\n").is_none());
        // 名前の一部が「リボン」で終わるだけの変数は拾わない
        assert!(decl_dict("横リボン = {\"札\": \"x\"}\n").is_some());
        assert!(decl_dict("設定 = {\"札\": \"x\"}\n").is_none());
    }

    #[test]
    fn リボンの名乗りは既定で埋まる() {
        let d = std::env::temp_dir().join(format!("owtest-ribbon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("締め.py"), "リボン = {\"札\": \"月次の締め\"}\n").unwrap();
        std::fs::write(d.join("名乗らない.py"), "def 走る():\n    pass\n").unwrap();
        let v = ribbon_decls(&d);
        assert_eq!(v.len(), 1, "名乗った .py だけがリボンに出る");
        assert_eq!(v[0].module, "締め");
        assert_eq!(v[0].label, "月次の締め");
        assert_eq!(v[0].tab, "マクロ", "段は既定でマクロ");
        assert_eq!(v[0].icon, "", "アイコンは空 — 既定に落とすのは呼ぶ側(icons を知らない)");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn リボンの宣言は普通の言葉のキーでも古いキーでも読める() {
        // 「札・絵・段」は普通の言葉ではないので「ラベル・アイコン・タブ」に
        // 言い換えました(2026-08-21)。**既に書いた .py が動かなくなると
        // 困る**ので、古いキーも読み続けます
        let d = std::env::temp_dir().join(format!("owtest-ribbon2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("新しい.py"),
            "リボン = {\"ラベル\": \"月次\", \"アイコン\": \"py-run\", \"タブ\": \"経理\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("古い.py"),
            "リボン = {\"札\": \"年次\", \"絵\": \"py-list\", \"段\": \"経理\"}\n",
        )
        .unwrap();
        std::fs::write(
            d.join("英語.py"),
            "リボン = {\"label\": \"Monthly\", \"icon\": \"py-run\", \"tab\": \"Books\"}\n",
        )
        .unwrap();
        let mut v = ribbon_decls(&d);
        v.sort_by(|a, b| a.module.cmp(&b.module));
        let 見た: Vec<_> = v
            .iter()
            .map(|r| (r.label.as_str(), r.icon.as_str(), r.tab.as_str()))
            .collect();
        assert_eq!(
            見た,
            vec![
                ("年次", "py-list", "経理"),   // 古い.py(札・絵・段)
                ("月次", "py-run", "経理"),    // 新しい.py(ラベル・アイコン・タブ)
                ("Monthly", "py-run", "Books"), // 英語.py
            ],
            "3つの書き方が同じように読めます"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 綴りの_venv_がいちばん強い() {
        // **開いているフォルダの .venv を最優先**(2026-08-24 発注者
        // 「zed と同じように作業ディレクトリー内の仮想環境を優先で」)。
        // 同じフォルダを JupyterLab とエディタと officework が見ているとき、
        // 3つとも同じ Python を使うのが期待される動きです
        let d = std::env::temp_dir().join(format!("owtest-workdir-{}", std::process::id()));
        let bin = if cfg!(windows) { d.join(".venv/Scripts") } else { d.join(".venv/bin") };
        std::fs::create_dir_all(&bin).unwrap();
        let py = if cfg!(windows) { bin.join("python.exe") } else { bin.join("python") };
        std::fs::write(&py, b"").unwrap();

        // 綴りを教える前は、ここには当たりません
        super::set_work_dir(None);
        assert_ne!(super::find_python(), py, "教えていないのに綴りを見た");

        // 教えたら、いちばん強い
        super::set_work_dir(Some(d.clone()));
        assert_eq!(super::find_python(), py, "綴りの .venv を見ていない");

        // **JO_PYTHON はさらに強い**(現場で差し替えられる、の決め)
        super::set_work_dir(None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 証明書の束を機械から探す() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // 配る Python は自分の径路を焼き付けているので、走らせる側が
        // 機械の束を教える(2026-08-14 に見本の天気予報で踏んだ)
        match super::ca_bundle() {
            Some(p) => assert!(p.exists(), "在ると言った物が無い: {}", p.display()),
            None => eprintln!("この機械には既知の置き場に証明書の束が無い(飛ばす)"),
        }
        // 既に指されていれば触らない(利用者の設定が勝つ)
        unsafe { std::env::set_var("SSL_CERT_FILE", "/tmp/わたしの束.pem") };
        assert!(super::py_env().is_empty(), "利用者の設定を上書きしている");
        unsafe { std::env::remove_var("SSL_CERT_FILE") };
    }

    /// 環境変数はプロセス全体で1つなので、それを書き換えるテストは
    /// **同時に走らせない**。下の2つは SSL_CERT_FILE を取り合っていて、
    /// 片方が設定して消す隙間にもう片方が子を起こすと、子には何も渡らない。
    /// 2026-08-17 に CI で落ちた(手元では 10 回に1回ほどしか出ない)
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn 証明書の道が子のプロセスに渡る() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // **実際に子を起こして確かめる。** py_env を書いただけでは、
        // run_with_timeout が渡し忘れていても気づけない(2026-08-14 に
        // 見本の天気予報が https で落ちて分かった穴)
        let Some(bundle) = super::ca_bundle() else {
            eprintln!("この機械に証明書の束が無い — 飛ばす");
            return;
        };
        unsafe { std::env::remove_var("SSL_CERT_FILE") };
        let mut c = std::process::Command::new("python3");
        c.args(["-c", "import os; print(os.environ.get('SSL_CERT_FILE', 'なし'))"]);
        match super::run_with_timeout(&mut c, 10) {
            Ok((true, out, _)) => assert_eq!(
                out.trim(),
                bundle.display().to_string(),
                "子に証明書の道が渡っていない"
            ),
            Ok((false, _, e)) => panic!("python3 が落ちた: {e}"),
            Err(_) => eprintln!("python3 が無い — 飛ばす"),
        }
    }
}

