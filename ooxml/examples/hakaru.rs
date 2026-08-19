//! docx を1つ受け取り、**adoc + テンプレートに分けたときの大きさを数える。**
//!
//! 発注者 2026-08-18「テンプレートと文書をわける。これが docx や odt, odp の
//! 複雑さを改善するのでは」。言い分ではなく数で答えるための道具です。
//!
//! ```bash
//! cargo run -p ooxml --example hakaru -- sample/writer/04_月次報告.docx
//! ```
//!
//! 出すのは、docx の部品の数と要素の数、分けた後の本文とテンプレートの
//! 字数・行数です。**中身が同じものを比べます** — 同じ文書を2つの形で
//! 表したときに、人が読む量がどれだけ違うかを見ます。

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("使い方: cargo run -p ooxml --example hakaru -- <docx>");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).expect("読めません");

    // HTML なら、読んで分けて、もう一度 HTML にするまでを見る
    if path.to_lowercase().ends_with(".html") || path.to_lowercase().ends_with(".htm") {
        return hakaru_html(&path, &bytes);
    }

    // ---- docx の側 ----
    let mut 部品 = Vec::new();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).expect("zip ではない");
    for i in 0..zip.len() {
        use std::io::Read;
        let mut f = zip.by_index(i).unwrap();
        let mut s = Vec::new();
        f.read_to_end(&mut s).ok();
        let 名 = f.name().to_string();
        // XML の要素の数(`<` の数から宣言と閉じを引く近似ではなく、開き札を数える)
        let text = String::from_utf8_lossy(&s);
        let 要素 = text.matches('<').count().saturating_sub(text.matches("</").count());
        部品.push((名, s.len(), 要素));
    }
    let 全要素: usize = 部品.iter().map(|p| p.2).sum();

    // ---- 分けた側 ----
    let (doc, _rep) = ooxml::read(std::io::Cursor::new(bytes)).expect("docx が読めません");
    let (本文, 型, r) = kumihan::distill::distill(&doc);
    let adoc = kumihan::adoc::write(&本文);
    let toml = kumihan::theme::write(&型);

    println!("== {path}");
    println!("docx: {} 個の部品、{} 個の XML の要素、{} バイト",
             部品.len(), 全要素, std::fs::metadata(&path).unwrap().len());
    let mut 大きい: Vec<_> = 部品.iter().filter(|p| p.2 > 0).collect();
    大きい.sort_by_key(|p| std::cmp::Reverse(p.2));
    for (名, _b, 要素) in 大きい.iter().take(5) {
        println!("      {要素:>6} 要素  {名}");
    }
    println!("分けた後:");
    println!("      本文(.adoc)     {:>5} 字 / {:>3} 行",
             adoc.chars().count(), adoc.lines().count());
    println!("      書式(.toml)     {:>5} 字 / {:>3} 行 / スタイル {} 個",
             toml.chars().count(), toml.lines().count(), 型.styles.len());
    println!("      落ちた所: {}", r.dropped);
}

/// HTML を受け取ったとき。**読む → 分ける → もう一度 HTML にする**まで見る。
///
/// Word が書き出した HTML は、見た目の指定が本文に絡みついている。分けると
/// どれだけ減るのか、そして**中身が残るのか**を数で見る。
fn hakaru_html(path: &str, bytes: &[u8]) {
    let src = String::from_utf8_lossy(bytes).to_string();
    println!("== {path}");
    println!("元の HTML: {} 行 / {} 字", src.lines().count(), src.chars().count());
    println!("      mso- の指定 {} 個、<span> {} 個、<o:p> {} 個",
             src.matches("mso-").count(),
             src.matches("<span").count(),
             src.matches("<o:p>").count());

    let (doc, 読めなかった) = kumihan::html::parse(&src);
    let 段落 = doc.paragraphs().count();
    let 表 = doc.tables().count();
    let 字: usize = doc.body_text().chars().count();
    println!("読んだ結果: 段落 {段落} 個 / 表 {表} 個 / 本文 {字} 字");
    if !読めなかった.is_empty() {
        println!("      読めなかったもの: {}", 読めなかった.join("・"));
    }

    let (本文, 型, r) = kumihan::distill::distill(&doc);
    let adoc = kumihan::adoc::write(&本文);
    let toml = kumihan::theme::write(&型);
    println!("分けた後: 本文 {} 行 / {} 字、書式 {} 行(スタイル {} 個)、落ちた所 {}",
             adoc.lines().count(), adoc.chars().count(),
             toml.lines().count(), 型.styles.len(), r.dropped);

    let page = kumihan::html_write::page(&本文, &型);
    println!("書き直した HTML: {} 行 / {} 字(元の {:.0}%)",
             page.html.lines().count(), page.html.chars().count(),
             page.html.chars().count() as f32 / src.chars().count() as f32 * 100.0);

    // **中身が残ったか。** 減らしただけで字が消えていたら意味がない
    let 元の字: String = strip(&src);
    let 後の字: String = strip(&page.html);
    let 残った = 元の字.chars().filter(|c| !c.is_whitespace()).count();
    let 後 = 後の字.chars().filter(|c| !c.is_whitespace()).count();
    println!("本文の字: 元 {残った} → 後 {後}");
    if let Some(dir) = std::env::args().skip(1).find(|a| a.starts_with("--out=")) {
        write_out(&dir["--out=".len()..], &adoc, &toml, &page.html);
    }
    for line in 元の字.lines().map(str::trim).filter(|l| l.len() > 6) {
        if !後の字.contains(line) {
            println!("      消えた行: {line}");
        }
    }
}

/// タグを外して字だけにする。**style と script と註釈の中は字ではない**
/// (見た目の指定や Word の控えなので、消えて当たり前のもの)
fn strip(s: &str) -> String {
    let mut rest = s;
    let mut body = String::new();
    // 註釈(Word の条件付き註釈もここで消える)
    while let Some(i) = rest.find("<!--") {
        body.push_str(&rest[..i]);
        rest = match rest[i..].find("-->") {
            Some(j) => &rest[i + j + 3..],
            None => "",
        };
    }
    body.push_str(rest);
    // style と script の中身
    for 札 in ["style", "script", "title"] {
        while let Some(i) = body.to_lowercase().find(&format!("<{札}")) {
            // 閉じ札が無ければ**そこで止める**(開いた札だけを消すと、
            // 中身が本文に紛れ込む)
            let Some(j) = body.to_lowercase()[i..].find(&format!("</{札}>")) else { break };
            body.replace_range(i..i + j + 札.len() + 3, "");
        }
    }
    let mut o = String::new();
    let mut in_tag = false;
    for c in body.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => o.push(c),
            _ => {}
        }
    }
    o.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
}

/// `--out <folder>` が付いていたら、分けた結果を書き出す(見るため)
fn write_out(dir: &str, adoc: &str, toml: &str, html: &str) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(format!("{dir}/本文.adoc"), adoc);
    let _ = std::fs::write(format!("{dir}/型.toml"), toml);
    let _ = std::fs::write(format!("{dir}/書き直し.html"), html);
    println!("書き出しました: {dir}");
}
