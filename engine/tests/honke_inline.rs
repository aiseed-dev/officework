//! **本家 asciidoctor の試験を写したもの(行の中の書き方)。**
//!
//! 元は `vendor/asciidoctor/test/substitutions_test.rb` の `Quotes`(92 本)
//! です。期待する答えは、本家を実際に動かして取りました
//! (`Asciidoctor.convert(入力, doctype: :inline)`、2026-09-02、2.1.0.alpha.0)。
//! 本家の HTML を、うちの run の並びを同じ形に写したもの([`sig`])と
//! 比べます。
//!
//! 本家に無い確認を1つ足しています。**読んだものを書き戻し、もう一度
//! 読んでも同じ並びになること**です。writer は視覚エディタなので、
//! 読めるだけでは足りません。
//!
//! 写さなかった試験と、うちの答えが本家と違う所は、
//! `docs/sekkei/asciidoctor-tsukiawase.ja.adoc` に書いてあります。
use kumihan::adoc;

mod common;
use common::{first_runs, sig};

/// 本家と同じ答えになる物。(本家の試験の名前, 入力, 本家の答えをうちの形にしたもの)
const CASES: &[(&str, &str, &str)] = &[
    ("single-line double-quoted string", "\"`a few quoted words`\"", "\"`a few quoted words`\""),
    ("escaped single-line double-quoted string 1", "\\\"`a few quoted words`\"", "\"`a few quoted words`\""),
    ("escaped single-line double-quoted string 2", "\\\\\"`a few quoted words`\"", "\\\"`a few quoted words`\""),
    ("multi-line double-quoted string", "\"`a few\nquoted words`\"", "\"`a few quoted words`\""),
    ("double-quoted string with inline single quote", "\"`Here's Johnny!`\"", "\"`Here's Johnny!`\""),
    ("double-quoted string with inline backquote", "\"`Here`s Johnny!`\"", "\"`Here`s Johnny!`\""),
    ("double-quoted string around monospaced text 2", "\"```E=mc^2^`` is the solution!`\"", "\"`<code>E=mc<sup>2</sup></code> is the solution!`\""),
    ("single-line single-quoted string", "'`a few quoted words`'", "'`a few quoted words`'"),
    ("escaped single-line single-quoted string", "\\'`a few quoted words`'", "'`a few quoted words`'"),
    ("multi-line single-quoted string", "'`a few\nquoted words`'", "'`a few quoted words`'"),
    ("single-quoted string with inline single quote", "'`That isn't what I did.`'", "'`That isn't what I did.`'"),
    ("single-quoted string with inline backquote", "'`Here`s Johnny!`'", "'`Here`s Johnny!`'"),
    ("single-line constrained marked string", "#a few words#", "<mark>a few words</mark>"),
    ("escaped single-line constrained marked string", "\\#a few words#", "#a few words#"),
    ("multi-line constrained marked string", "#a few\nwords#", "<mark>a few words</mark>"),
    ("constrained marked string should not match entity references", "111 #mark a# 222 \"`quote a`\" 333 #mark b# 444", "111 <mark>mark a</mark> 222 \"`quote a`\" 333 <mark>mark b</mark> 444"),
    ("single-line unconstrained marked string", "##--anything goes ##", "<mark>--anything goes </mark>"),
    ("escaped single-line unconstrained marked string", "\\\\##--anything goes ##", "##--anything goes ##"),
    ("multi-line unconstrained marked string", "##--anything\ngoes ##", "<mark>--anything goes </mark>"),
    ("single-line constrained marked string with role", "[statement]#a few words#", "<span class=\"statement\">a few words</span>"),
    ("does not recognize attribute list with left square bracket on formatted text", "key: [ *before [.redacted]#redacted# after* ]", "key: [ <strong>before <span class=\"redacted\">redacted</span> after</strong> ]"),
    ("should ignore enclosing square brackets when processing formatted text with attribute list", "nums = [1, 2, 3, [.blue]#4#]", "nums = [1, 2, 3, <span class=\"blue\">4</span>]"),
    ("single-line constrained strong string", "*a few strong words*", "<strong>a few strong words</strong>"),
    ("escaped single-line constrained strong string", "\\*a few strong words*", "*a few strong words*"),
    ("multi-line constrained strong string", "*a few\nstrong words*", "<strong>a few strong words</strong>"),
    ("constrained strong string containing an asterisk", "*bl*ck*-eye", "<strong>bl*ck</strong>-eye"),
    ("constrained strong string containing an asterisk and multibyte word chars", "*黑*眼圈*", "<strong>黑*眼圈</strong>"),
    ("single-line constrained quote variation emphasized string", "_a few emphasized words_", "<em>a few emphasized words</em>"),
    ("escaped single-line constrained quote variation emphasized string", "\\_a few emphasized words_", "_a few emphasized words_"),
    ("escaped single quoted string", "\\'a few emphasized words'", "\\'a few emphasized words'"),
    ("multi-line constrained emphasized quote variation string", "_a few\nemphasized words_", "<em>a few emphasized words</em>"),
    ("single-quoted string containing an emphasized phrase", "'`I told him, 'Just go for it!'`'", "'`I told him, 'Just go for it!'`'"),
    ("single-line constrained emphasized underline variation string", "_a few emphasized words_", "<em>a few emphasized words</em>"),
    ("escaped single-line constrained emphasized underline variation string", "\\_a few emphasized words_", "_a few emphasized words_"),
    ("multi-line constrained emphasized underline variation string", "_a few\nemphasized words_", "<em>a few emphasized words</em>"),
    ("escaped single-line constrained monospaced string", "\\`a few <monospaced> words`", "`a few <monospaced> words`"),
    ("escaped single-line constrained monospaced string with role", "[input]\\`a few <monospaced> words`", "[input]`a few <monospaced> words`"),
    ("escaped role on single-line constrained monospaced string", "\\[input]`a few <monospaced> words`", "[input]<code>a few <monospaced> words</code>"),
    ("escaped role on escaped single-line constrained monospaced string", "\\[input]\\`a few <monospaced> words`", "\\[input]`a few <monospaced> words`"),
    ("escaped single-line constrained monospace string with forced compat role", "[x-]\\`leave it alone`", "[x-]`leave it alone`"),
    ("escaped forced compat role on single-line constrained monospace string", "\\[x-]`just *mono*`", "[x-]<code>just <strong>mono</strong></code>"),
    ("single-line unconstrained strong chars", "**Git**Hub", "<strong>Git</strong>Hub"),
    ("escaped single-line unconstrained strong chars", "\\**Git**Hub", "<strong>*Git</strong>*Hub"),
    ("multi-line unconstrained strong chars", "**G\ni\nt\n**Hub", "<strong>G i t </strong>Hub"),
    ("unconstrained strong chars with inline asterisk", "**bl*ck**-eye", "<strong>bl*ck</strong>-eye"),
    ("unconstrained strong chars with role", "Git[blue]**Hub**", "Git<strong class=\"blue\">Hub</strong>"),
    ("escaped unconstrained strong chars with role", "Git\\[blue]**Hub**", "Git[blue]<strong>*Hub</strong>*"),
    ("single-line unconstrained emphasized chars", "__Git__Hub", "<em>Git</em>Hub"),
    ("escaped single-line unconstrained emphasized chars", "\\__Git__Hub", "__Git__Hub"),
    ("escaped single-line unconstrained emphasized chars around word", "\\\\__GitHub__", "__GitHub__"),
    ("multi-line unconstrained emphasized chars", "__G\ni\nt\n__Hub", "<em>G i t </em>Hub"),
    ("unconstrained emphasis chars with role", "[gray]__Git__Hub", "<em class=\"gray\">Git</em>Hub"),
    ("escaped unconstrained emphasis chars with role", "\\[gray]__Git__Hub", "[gray]__Git__Hub"),
    ("single-line constrained monospaced chars 1", "call [x-]+save()+ to persist the changes", "call <code>save()</code> to persist the changes"),
    ("single-line constrained monospaced chars 2", "call `save()` to persist the changes", "call <code>save()</code> to persist the changes"),
    ("escaped single-line constrained monospaced chars", "call \\`save()` to persist the changes", "call `save()` to persist the changes"),
    ("escaped single-line constrained monospaced chars with role", "call [method]\\`save()` to persist the changes", "call [method]`save()` to persist the changes"),
    ("escaped role on single-line constrained monospaced chars", "call \\[method]`save()` to persist the changes", "call [method]<code>save()</code> to persist the changes"),
    ("escaped role on escaped single-line constrained monospaced chars", "call \\[method]\\`save()` to persist the changes", "call \\[method]`save()` to persist the changes"),
    ("escaped single-line constrained passthrough string with forced compat role", "[x-]\\+leave it alone+", "[x-]+leave it alone+"),
    ("single-line unconstrained monospaced chars 1", "Git[x-]++Hub++", "Git<code>Hub</code>"),
    ("single-line unconstrained monospaced chars 2", "Git``Hub``", "Git<code>Hub</code>"),
    ("escaped single-line unconstrained monospaced chars", "Git\\``Hub``", "Git``Hub``"),
    ("multi-line unconstrained monospaced chars 1", "Git[x-]++\nH\nu\nb++", "Git<code> H u b</code>"),
    ("multi-line unconstrained monospaced chars 2", "Git``\nH\nu\nb``", "Git<code> H u b</code>"),
    ("single-line superscript chars", "x^2^ = x * x, e = mc^2^, there's a 1^st^ time for everything", "x<sup>2</sup> = x * x, e = mc<sup>2</sup>, there's a 1<sup>st</sup> time for everything"),
    ("escaped single-line superscript chars", "x\\^2^ = x * x", "x^2^ = x * x"),
    ("does not match superscript across whitespace", "x^(n\n-\n1)^", "x^(n - 1)^"),
    ("allow spaces in superscript if text is wrapped in a passthrough", "Night ^+A poem by Jane Kondo+^.", "Night <sup>A poem by Jane Kondo</sup>."),
    ("does not match adjacent superscript chars", "a ^^ b", "a ^^ b"),
    ("single-line subscript chars", "H~2~O", "H<sub>2</sub>O"),
    ("escaped single-line subscript chars", "H\\~2~O", "H~2~O"),
    ("does not match subscript across whitespace", "project~ view\non\nGitHub~", "project~ view on GitHub~"),
    ("does not match adjacent subscript chars", "a ~~ b", "a ~~ b"),
    ("does not match subscript across distinct URLs", "http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]", "<a href=\"http://www.abc.com/~def\">DEF</a> and <a href=\"http://www.abc.com/~ghi\">GHI</a>"),
    ("quoted text with role shorthand", "[.white.red-background]#alert#", "<span class=\"white red-background\">alert</span>"),
    ("quoted text with id shorthand", "[#bond]#007#", "007"),
    ("quoted text with id and role shorthand", "[#bond.white.red-background]#007#", "<span class=\"white red-background\">007</span>"),
    ("quoted text with id and role shorthand with roles before id", "[.white.red-background#bond]#007#", "<span class=\"white red-background\">007</span>"),
    ("quoted text with id and role shorthand with roles around id", "[.white#bond.red-background]#007#", "<span class=\"white red-background\">007</span>"),
    ("should not assign role attribute if shorthand style has no roles", "[#idname]*blah*", "<strong>blah</strong>"),
    ("should remove trailing spaces from role defined using shorthand", "[.rolename ]*blah*", "<strong class=\"rolename\">blah</strong>"),
    ("should ignore attributes after comma", "[red, foobar]#alert#", "<span class=\"red\">alert</span>"),
    ("should remove leading and trailing spaces around role after ignoring attributes after comma", "[ red , foobar]#alert#", "<span class=\"red\">alert</span>"),
    ("should not assign role if value before comma is empty", "[,]#anonymous#", "anonymous"),
    ("inline passthrough with id and role set using shorthand 1", "[#idname.rolename]+pass+", "<span class=\"rolename\">pass</span>"),
    ("inline passthrough with id and role set using shorthand 2", "[.rolename#idname]+pass+", "<span class=\"rolename\">pass</span>"),
];

/// 本家と答えが違う物。(名前, 入力, 本家, うち, 違う理由)。
/// うちの答えを固定しておき、黙って変わらないようにします
const DIFFERS: &[(&str, &str, &str, &str, &str)] = &[
    ("does not confuse superscript and links with blank window shorthand", "http://localhost[Text^] on the 21^st^ and 22^nd^", "<a href=\"http://localhost\">Text</a> on the 21<sup>st</sup> and 22<sup>nd</sup>", "<a href=\"http://localhost\">Text^</a> on the 21<sup>st</sup> and 22<sup>nd</sup>", "別の窓で開く印 `^` は模型に無い。字のまま残して書き戻す(落とすと本家の出力が変わる)"),
    ("double-quoted string around monospaced text 1", "\"``E=mc^2^` is the solution!`\"", "\"``E=mc<sup>2</sup>` is the solution!`\"", "\"`<code>E=mc<sup>2</sup></code> is the solution!`\"", "本家は HTML の実体参照(&#8220;)の `;` が等幅の開きを止める。うちは HTML を経ないので等幅になる"),
    ("escaped single-quotes inside emphasized words are restored", "'Here\\'s Johnny!'", "'Here's Johnny!'", "'Here\\'s Johnny!'", "置き換え(replacements)の話。うちは読むときに字を変えない"),
    ("single-line constrained monospaced string", "`a few <{monospaced}> words`", "<code>a few <monospaced> words</code>", "<code>a few <{monospaced}> words</code>", "属性の参照(`{monospaced}`)は別の回で扱う"),
    ("single-line constrained monospaced string with role", "[input]`a few <{monospaced}> words`", "<code class=\"input\">a few <monospaced> words</code>", "<span class=\"input\">a few <{monospaced}> words</span>", "等幅と役割は1つの run に同居できない(style_id が1つ)。役割を採る。属性の参照も残る"),
    ("should ignore role that ends with transitional role on constrained monospace span", "[foox-]`leave it alone`", "<code class=\"foox-\">leave it alone</code>", "<span class=\"foox-\">leave it alone</span>", "等幅と役割は1つの run に同居できない(style_id が1つ)。役割を採る"),
    ("multi-line constrained monospaced string", "`a few\n<{monospaced}> words`", "<code>a few <monospaced> words</code>", "<code>a few <{monospaced}> words</code>", "属性の参照(`{monospaced}`)は別の回で扱う"),
    ("single-line constrained monospaced chars with role 1", "call [method x-]+save()+ to persist the changes", "call <code class=\"method\">save()</code> to persist the changes", "call <span class=\"method\">save()</span> to persist the changes", "等幅と役割は1つの run に同居できない(style_id が1つ)。役割を採る"),
    ("single-line constrained monospaced chars with role 2", "call [method]`save()` to persist the changes", "call <code class=\"method\">save()</code> to persist the changes", "call <span class=\"method\">save()</span> to persist the changes", "等幅と役割は1つの run に同居できない(style_id が1つ)。役割を採る"),
];

/// 往復の確認から外す物。(名前, 理由)
const NO_ROUND_TRIP: &[(&str, &str)] = &[
    ("escaped single-line unconstrained strong chars", "本家の癖(`\\**` が `<strong>*Git</strong>*` になる)。太字の頭の `*` を書くには passthrough が要る"),
    ("escaped unconstrained strong chars with role", "同上"),
];

#[test]
fn quotes_read_like_upstream() {
    let mut bad = Vec::new();
    for (name, src, want) in CASES {
        let d = adoc::parse(&format!("{src}\n")).expect(name);
        let got = sig(&first_runs(&d));
        if got != *want {
            bad.push(format!("{name}\n  入力: {src:?}\n  本家: {want}\n  うち: {got}"));
        }
    }
    assert!(bad.is_empty(), "{} 本が本家と違う:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn the_known_differences_stay_as_recorded() {
    let mut bad = Vec::new();
    for (name, src, honke, ours, why) in DIFFERS {
        let d = adoc::parse(&format!("{src}\n")).expect(name);
        let got = sig(&first_runs(&d));
        if got == *honke {
            bad.push(format!("{name}: 本家と同じになった。DIFFERS から CASES へ移すこと"));
        } else if got != *ours {
            bad.push(format!("{name}({why})\n  入力: {src:?}\n  記録: {ours}\n  いま: {got}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// 書き戻して読み直しても同じ並び(本家に無い、うちだけの確認)
#[test]
fn quotes_survive_a_write_and_a_second_read() {
    let mut bad = Vec::new();
    let all = CASES.iter().map(|(n, s, _)| (*n, *s)).chain(DIFFERS.iter().map(|(n, s, ..)| (*n, *s)));
    for (name, src) in all {
        if NO_ROUND_TRIP.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let d = adoc::parse(&format!("{src}\n")).expect(name);
        let once = sig(&first_runs(&d));
        let back = adoc::write(&d);
        let d2 = adoc::parse(&back).expect(name);
        let twice = sig(&first_runs(&d2));
        if once != twice {
            bad.push(format!("{name}\n  入力: {src:?}\n  書き戻し: {back:?}\n  1度目: {once}\n  2度目: {twice}"));
        }
    }
    assert!(bad.is_empty(), "{} 本が往復で変わる:\n{}", bad.len(), bad.join("\n"));
}
