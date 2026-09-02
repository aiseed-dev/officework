//! **本家 asciidoctor の試験を写したもの(字の基本)。**
//!
//! 元は `vendor/asciidoctor/test/text_test.rb` です。入力は本家の試験を読み込んで
//! 記録し、答えは本家を実際に動かして取りました(2026-09-02、2.1.0.alpha.0)。
//! 1つの段落になる入力だけを写しています。節や複数の段落が要る物は
//! `SKIPPED` に名前と理由を残しました。
//!
//! 本家と答えが違う物は `DIFFERS` に理由を付けて記録し、うちの答えを
//! 固定します。写さなかった物と違う理由の一覧は
//! `docs/sekkei/asciidoctor-tsukiawase.ja.adoc` にあります。
use kumihan::adoc;

mod common;
use common::{first_runs, sig};

/// 本家と同じ答えになる物。(本家の試験の名前, 入力, 本家の答えをうちの形にしたもの)
const CASES: &[(&str, &str, &str)] = &[
    ("escaped text markup", "All your <em>inline</em> markup belongs to <strong>us</strong>!", "All your <em>inline</em> markup belongs to <strong>us</strong>!"),
    ("line breaks", "Well this is +\njust fine and dandy, isn't it?", "Well this is\njust fine and dandy, isn't it?"),
    ("single- and double-quoted text", "\"`Where?,`\" she said, flipping through her copy of '`The New Yorker.`'", "\"`Where?,`\" she said, flipping through her copy of '`The New Yorker.`'"),
    ("multiple double-quoted text on a single line", "\"`Our business is constantly changing`\" or \"`We need faster time to market.`\"", "\"`Our business is constantly changing`\" or \"`We need faster time to market.`\""),
    ("emphasized text using underscore characters", "An _emphatic_ no", "An <em>emphatic</em> no"),
    ("emphasized text with single quote using apostrophe characters", "It's 'Johnny's' phone", "It's 'Johnny's' phone"),
    ("unescape escaped single quote emphasis in compat mode only", "A \\'single quoted string' example", "A \\'single quoted string' example"),
    ("unescape escaped single quote emphasis in compat mode only 2", "\\'single quoted string'", "\\'single quoted string'"),
    ("emphasized text at end of line", "This library is _awesome_", "This library is <em>awesome</em>"),
    ("emphasized text at beginning of line", "_drop_ it", "<em>drop</em> it"),
    ("emphasized text across line", "_check it_", "<em>check it</em>"),
    ("unquoted text", "An #unquoted# word", "An <mark>unquoted</mark> word"),
    ("backticks and straight quotes in text", "run `foo` 'dog'", "run <code>foo</code> 'dog'"),
    ("backticks and straight quotes in text 2", "run \\`foo` 'dog'", "run `foo` 'dog'"),
    ("backticks and straight quotes in text 3", "run '`foo` 'dog`'", "run '`foo` 'dog`'"),
    ("plus characters inside single plus passthrough", "+++", "+"),
    ("plus characters inside single plus passthrough 2", "++=+", "+="),
    ("plus passthrough escapes entity reference", "+&#44;+", "&#44;"),
    ("plus passthrough escapes entity reference 2", "one++&#44;++two", "one&#44;two"),
    ("passthrough", "This is +passed through+.", "This is passed through."),
    ("nested styles", "Winning *big _time_* in the `city *boyeeee*`.", "Winning <strong>big <em>time</em></strong> in the <code>city <strong>boyeeee</strong></code>."),
    ("should format Asian characters as words", "bold *要* bold", "bold <strong>要</strong> bold"),
    ("should format Asian characters as words 2", "bold *素* bold", "bold <strong>素</strong> bold"),
    ("should format Asian characters as words 3", "bold *要素* bold", "bold <strong>要素</strong> bold"),
];

/// 本家と答えが違う物。(名前, 入力, 本家, うち, 違う理由)
const DIFFERS: &[(&str, &str, &str, &str, &str)] = &[
    ("emphasized text with escaped single quote using apostrophe characters", "It\\'s 'Johnny\\'s' phone", "It's 'Johnny's' phone", "It\\'s 'Johnny\\'s' phone", "置き換え(replacements)の話。うちは読むときに字を変えない"),
    ("escaped single quote is restored as single quote", "Let\\'s do it!", "Let's do it!", "Let\\'s do it!", "置き換え(replacements)の話。うちは読むときに字を変えない"),
    ("backticks and straight quotes in text 4", "run \\'`foo` 'dog\\`'", "run '`foo` 'dog`'", "run '`foo` 'dog\\`'", "置き換え(replacements)の話。うちは読むときに字を変えない"),
    ("unconstrained quotes", "**B**__I__``M``[role]``M``", "<strong>B</strong><em>I</em><code>M</code><code class=\"role\">M</code>", "<strong>B</strong><em>I</em><code>M</code><span class=\"role\">M</span>", "等幅と役割は1つの run に同居できない(style_id が1つ)。役割を採る"),
];

/// 写さなかった物。(名前, 理由)
#[allow(dead_code)]
const SKIPPED: &[(&str, &str)] = &[
    ("horizontal rule", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 2", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 3", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 4", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 5", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 6", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 7", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 8", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 9", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 10", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 11", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 12", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 13", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 14", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 15", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 16", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 17", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 18", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 19", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 20", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 21", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 22", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 23", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules 24", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 2", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 3", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 4", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 5", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 6", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 7", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 8", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 9", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 10", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 11", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 12", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 13", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 14", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 15", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 16", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 17", "文書の構造(節・複数の段落)が要る"),
    ("markdown horizontal rules negative case 18", "文書の構造(節・複数の段落)が要る"),
];

/// 往復の確認から外す物。(名前, 理由)
const NO_ROUND_TRIP: &[(&str, &str)] = &[
];

#[test]
fn read_like_upstream() {
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
fn survive_a_write_and_a_second_read() {
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
