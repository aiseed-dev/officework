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

// ---- plugins(.py の置き場)-------------------------------------------------

/// プラグイン(.py)の置き場。~/.config/office/plugins。
/// **ここが正** — ui::pyedit と calc/writer は包みで呼ぶ
pub fn plugins_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/office/plugins")
}

/// plugins にある .py の名前(モジュール名)を並べる。
pub fn plugin_modules() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(plugins_dir())
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
    plugin_modules()
        .into_iter()
        .map(|m| {
            let src =
                std::fs::read_to_string(plugins_dir().join(format!("{m}.py"))).unwrap_or_default();
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
    let mut v: Vec<_> = plugin_modules()
        .into_iter()
        .filter_map(|m| {
            let md = std::fs::metadata(plugins_dir().join(format!("{m}.py"))).ok()?;
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

/// 同梱の Python(配る形に入れる時の置き場)。実行ファイルの隣の
/// `python/` を見る — Windows は `python/python.exe`、他は `python/bin/python3`。
///
/// **配るときは Python を同梱する**(発注者 2026-08-14。Windows は
/// Python が入っていない機械が普通で、同梱しないと「動かない」から
/// 始まる。Flet も同じ形)。中身は python-build-standalone
/// (astral-sh。PSF ライセンスで再配布できる)の **3.14 系** — 手元の
/// miniforge3 と揃える。3.12 ではスマホの的に届かない(発注者)。
/// pip も入っているので、matplotlib や polars は同梱の python に
/// 後から入れられる
fn bundled_python(exe_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let cands = if cfg!(windows) {
        ["python/python.exe", "python.exe"]
    } else {
        ["python/bin/python3", "python/bin/python"]
    };
    cands
        .iter()
        .map(|c| exe_dir.join(c))
        .find(|p| p.exists())
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

/// 裏方の Python を探す。**JO_PYTHON → 同梱 → .venv → python3**。
/// matplotlib が居るかは実行して分かる(居なければ status で言う)。
pub fn find_python() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("JO_PYTHON") {
        return p.into();
    }
    // 配る形に同梱した Python(実行ファイルの隣)。**.venv より先に見る** —
    // 配った物は同梱を使うのが筋で、開発機の .venv に引っ張られない
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = bundled_python(dir) {
                return p;
            }
        }
    }
    // 今いるフォルダの .venv(リポジトリ直下で起動した形)
    let venv = std::path::Path::new(".venv/bin/python");
    if venv.exists() {
        return venv.into();
    }
    // 実行ファイルの場所から遡って .venv を探す(target/release/calc →
    // リポジトリ直下)。**どこから起動しても同じ python に当たる** —
    // CWD 頼みだと「polars がありません」になり、ピボットが置けない
    // (発注者の実機で踏んだ 2026-08-07)
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1) {
            let p = dir.join(".venv/bin/python");
            if p.exists() {
                return p;
            }
        }
    }
    "python3".into()
}

// ---- 裏方の台本(呼ぶ側がデータを JSON 等で渡す)----------------------------

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
for i, s in enumerate(series):
    ax.bar(x + (i - (n - 1) / 2) * w, s["values"], w, label=s["name"])
ax.set_xticks(x)
ax.set_xticklabels(labels)
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
r = linprog(
    c=spec["c"],
    A_ub=spec["aub"] or None,
    b_ub=spec["bub"] or None,
    A_eq=spec["aeq"] or None,
    b_eq=spec["beq"] or None,
    bounds=[(lo, None)] * n,
    method="highs",
)
if not r.success:
    sys.exit("解がありません: " + str(r.message))
sys.stdout.write("\x1f".join("%.12g" % v for v in r.x))
"#;

/// ピボットの台本(polars)。指図は JSON、答えは CSV 取り込みと同じ
/// 区切りの印(\x1e 行 / \x1f 欄)で返す。
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
    if unit == "年":
        out = d.dt.strftime("%Y年")
    elif unit == "四半期":
        out = (d.dt.year().cast(pl.String) + "年Q" +
               ((d.dt.month() + 2) // 3).cast(pl.String))
    else:  # 月
        out = d.dt.strftime("%Y-%m")
    # 日付として読めない値はそのまま残す(黙って落とさない)
    return pl.when(d.is_null()).then(pl.col(col)).otherwise(out)

for _f, _u in spec.get("group", []):
    if _f in df.columns:
        df = df.with_columns(_grouped(_f, _u).alias(_f))
val, agg = spec["value"], spec["agg"]
if agg != "個数":
    # 数にならないものは null(集計から外れる)
    df = df.with_columns(pl.col(val).cast(pl.Float64, strict=False))
idx, cols = spec["index"], spec["columns"]
FN = {"合計": "sum", "平均": "mean", "個数": "len", "最大": "max", "最小": "min"}

def agg_expr():
    return {"合計": pl.sum(val), "平均": pl.mean(val), "個数": pl.len().alias(val),
            "最大": pl.max(val), "最小": pl.min(val)}[agg]

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
        cells = [f"{g} 小計"] + [""] * (len(idx) - 1) + sub[g]
        if tot_col:
            cells.append(sub_tots.get((g,)))
        out.append(("s", cells))
    if spec["blank_rows"] and len(idx) >= 2:
        out.append(("b", [""] * (len(main.columns) + (1 if tot_col else 0))))

if spec["totals"] and df.height:
    cells = list(table(stub(df, "総計", idx), idx).rows()[0])
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

head = list(main.columns) + (["総計"] if tot_col else [])
lines = []
if cols:
    # Excel と同じ1行目の札: 「合計 / 金額」と、列に広げた見出し(月)
    label = [f"{agg} / {val}"] + [""] * (len(idx) - 1) + [" / ".join(cols)]
    label += [""] * (len(head) - len(label))
    lines.append("l\x1f" + "\x1f".join(label))
else:
    # 列が無いときは値の列の見出しを「合計 / 金額」に(Excel と同じ)
    head[-2 if tot_col else -1] = f"{agg} / {val}"
lines.append("h\x1f" + "\x1f".join(head))
for kind, cells in out:
    lines.append(kind + "\x1f" + "\x1f".join(s(v) for v in cells))
sys.stdout.buffer.write("\x1e".join(lines).encode("utf-8"))
"#;

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
    fn 同梱の_python_を実行ファイルの隣から見つける() {
        // 配る形は python/ を実行ファイルの隣に置く(2026-08-14)。
        // Windows は python/python.exe、他は python/bin/python3
        let d = std::env::temp_dir().join(format!("owtest-bundled-{}", std::process::id()));
        let sub = if cfg!(windows) { d.join("python") } else { d.join("python/bin") };
        std::fs::create_dir_all(&sub).unwrap();
        assert!(super::bundled_python(&d).is_none(), "無い時に見つけてはいけない");
        let exe = if cfg!(windows) { sub.join("python.exe") } else { sub.join("python3") };
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(super::bundled_python(&d), Some(exe), "隣の python を見つけていない");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn 証明書の束を機械から探す() {
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

    #[test]
    fn 証明書の道が子のプロセスに渡る() {
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
