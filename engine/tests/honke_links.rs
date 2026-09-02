//! **本家 asciidoctor の試験を写したもの(リンクと参照)。**
//!
//! 元は `vendor/asciidoctor/test/links_test.rb` です。入力は本家の試験を読み込んで
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
    ("qualified url inline with text", "The AsciiDoc project is located at http://asciidoc.org.", "The AsciiDoc project is located at <a href=\"http://asciidoc.org\">http://asciidoc.org</a>."),
    ("qualified url with role inline with text", "The AsciiDoc project is located at http://asciidoc.org[role=project].", "The AsciiDoc project is located at <a href=\"http://asciidoc.org\" class=\"project\">http://asciidoc.org</a>."),
    ("qualified file url inline with label", "file:///home/user/bookmarks.html[My Bookmarks]", "<a href=\"file:///home/user/bookmarks.html\">My Bookmarks</a>"),
    ("qualified url with label", "We're parsing http://asciidoc.org[AsciiDoc] markup", "We're parsing <a href=\"http://asciidoc.org\">AsciiDoc</a> markup"),
    ("qualified url with label containing escaped right square bracket", "We're parsing http://asciidoc.org[[Ascii\\]Doc] markup", "We're parsing <a href=\"http://asciidoc.org\">[Ascii]Doc</a> markup"),
    ("qualified url with backslash label", "I advise you to https://google.com[Google for +\\+]", "I advise you to <a href=\"https://google.com\">Google for \\</a>"),
    ("qualified url with label using link macro", "We're parsing link:http://asciidoc.org[AsciiDoc] markup", "We're parsing <a href=\"http://asciidoc.org\">AsciiDoc</a> markup"),
    ("qualified url with role using link macro", "We're parsing link:http://asciidoc.org[role=project] markup", "We're parsing <a href=\"http://asciidoc.org\" class=\"project\">http://asciidoc.org</a> markup"),
    ("qualified url using macro syntax with multi-line label inline with text", "We're parsing link:http://asciidoc.org[AsciiDoc\nmarkup]", "We're parsing <a href=\"http://asciidoc.org\">AsciiDoc markup</a>"),
    ("link macro with empty target", "Link to link:[this page].", "Link to <a href=\"\">this page</a>."),
    ("should not recognize link macro with double colons", "The link::http://example.org[example domain] is reserved for tests and documentation.", "The link::http://example.org[example domain] is reserved for tests and documentation."),
    ("qualified url surrounded by angled brackets", "<http://asciidoc.org> is the project page for AsciiDoc.", "<a href=\"http://asciidoc.org\">http://asciidoc.org</a> is the project page for AsciiDoc."),
    ("qualified url surrounded by double angled brackets should preserve outer angled brackets", "<<https://asciidoc.org>>", "<<a href=\"https://asciidoc.org\">https://asciidoc.org</a>>"),
    ("qualified url macro inside angled brackets", "<https://asciidoc.org[]>", "<<a href=\"https://asciidoc.org\">https://asciidoc.org</a>>"),
    ("qualified url surrounded by angled brackets in unconstrained context", "URLは<http://asciidoc.org>。fin", "URLは<a href=\"http://asciidoc.org\">http://asciidoc.org</a>。fin"),
    ("multiple qualified urls surrounded by angled brackets in unconstrained context", "URLは<http://asciidoc.org>。URLは<http://asciidoc.org>。", "URLは<a href=\"http://asciidoc.org\">http://asciidoc.org</a>。URLは<a href=\"http://asciidoc.org\">http://asciidoc.org</a>。"),
    ("qualified url surrounded by escaped angled brackets should escape form", "\\<http://asciidoc.org>", "<http://asciidoc.org>"),
    ("escaped qualified url surrounded by angled brackets should escape autolink", "<\\http://asciidoc.org>", "<http://asciidoc.org>"),
    ("xref shorthand with target that starts with URL protocol and has space after comma should not crash parser", "<<https://example.com, Example>>", "<a href=\"#https://example.com\">Example</a>"),
    ("xref shorthand with link macro as target should be ignored", "<<link:https://example.com[], Example>>", "<<<a href=\"https://example.com\">https://example.com</a>, Example>>"),
    ("autolink containing text enclosed in angle brackets", "https://github.com/<org>/", "<a href=\"https://github.com/<org>/\">https://github.com/<org>/</a>"),
    ("qualified url surrounded by round brackets", "(http://asciidoc.org) is the project page for AsciiDoc.", "(<a href=\"http://asciidoc.org\">http://asciidoc.org</a>) is the project page for AsciiDoc."),
    ("qualified url with trailing period", "The homepage for Asciidoctor is https://asciidoctor.org.", "The homepage for Asciidoctor is <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>."),
    ("qualified url with trailing explanation point", "Check out https://asciidoctor.org!", "Check out <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>!"),
    ("qualified url with trailing question mark", "Is the homepage for Asciidoctor https://asciidoctor.org?", "Is the homepage for Asciidoctor <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>?"),
    ("qualified url with trailing round bracket", "Asciidoctor is a Ruby-based AsciiDoc processor (see https://asciidoctor.org)", "Asciidoctor is a Ruby-based AsciiDoc processor (see <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>)"),
    ("qualified url with trailing period followed by round bracket", "(The homepage for Asciidoctor is https://asciidoctor.org.)", "(The homepage for Asciidoctor is <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>.)"),
    ("qualified url with trailing exclamation point followed by round bracket", "(Check out https://asciidoctor.org!)", "(Check out <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>!)"),
    ("qualified url with trailing question mark followed by round bracket", "(Is the homepage for Asciidoctor https://asciidoctor.org?)", "(Is the homepage for Asciidoctor <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>?)"),
    ("qualified url with trailing semi-colon", "https://asciidoctor.org; where text gets parsed", "<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>; where text gets parsed"),
    ("qualified url with trailing colon", "https://asciidoctor.org: where text gets parsed", "<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>: where text gets parsed"),
    ("qualified url in round brackets with trailing colon", "(https://asciidoctor.org): where text gets parsed", "(<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>): where text gets parsed"),
    ("qualified url with trailing round bracket followed by colon", "(from https://asciidoctor.org): where text gets parsed", "(from <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>): where text gets parsed"),
    ("qualified url in round brackets with trailing semi-colon", "(https://asciidoctor.org); where text gets parsed", "(<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>); where text gets parsed"),
    ("qualified url with trailing round bracket followed by semi-colon", "(from https://asciidoctor.org); where text gets parsed", "(from <a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>); where text gets parsed"),
    ("URI scheme with trailing characters should not be converted to a link", "http://;", "http://;"),
    ("URI scheme with trailing characters should not be converted to a link 2", "file://:", "file://:"),
    ("URI scheme with trailing characters should not be converted to a link 3", "irc://,", "irc://,"),
    ("bare URI scheme enclosed in brackets should not be converted to link", "(https://)", "(https://)"),
    ("bare URI scheme enclosed in brackets should not be converted to link 2", "<ftp://>", "<ftp://>"),
    ("qualified url containing round brackets", "http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)[addModule() adds a Ruby module]", "<a href=\"http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)\">addModule() adds a Ruby module</a>"),
    ("qualified url adjacent to text in square brackets", "]http://asciidoc.org[AsciiDoc] project page.", "]<a href=\"http://asciidoc.org\">AsciiDoc</a> project page."),
    ("qualified url adjacent to text in round brackets", ")http://asciidoc.org[AsciiDoc] project page.", ")<a href=\"http://asciidoc.org\">AsciiDoc</a> project page."),
    ("qualified url following no-break space", " http://asciidoc.org[AsciiDoc] project page.", " <a href=\"http://asciidoc.org\">AsciiDoc</a> project page."),
    ("qualified url following smart apostrophe", "l&#8217;http://www.irit.fr[IRIT]", "l&#8217;<a href=\"http://www.irit.fr\">IRIT</a>"),
    ("should convert qualified url as macro enclosed in double quotes", "\"https://asciidoctor.org[]\"", "\"<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>\""),
    ("should convert qualified url as macro enclosed in single quotes", "'https://asciidoctor.org[]'", "'<a href=\"https://asciidoctor.org\">https://asciidoctor.org</a>'"),
    ("should convert qualified url as macro with trailing period", "Information about the https://symbols.example.org/.[.] character.", "Information about the <a href=\"https://symbols.example.org/.\">.</a> character."),
    ("qualified url using invalid link macro should not create link", "link:http://asciidoc.org is the project page for AsciiDoc.", "link:http://asciidoc.org is the project page for AsciiDoc."),
    ("escaped inline qualified url should not create link", "\\http://asciidoc.org is the project page for AsciiDoc.", "http://asciidoc.org is the project page for AsciiDoc."),
    ("escaped inline qualified url as macro should not create link", "\\http://asciidoc.org[asciidoc.org] is the project page for AsciiDoc.", "http://asciidoc.org[asciidoc.org] is the project page for AsciiDoc."),
    ("url in link macro with at (@) sign should not create mailto link", "http://xircles.codehaus.org/lists/dev@geb.codehaus.org[subscribe]", "<a href=\"http://xircles.codehaus.org/lists/dev@geb.codehaus.org\">subscribe</a>"),
    ("implicit url with at (@) sign should not create mailto link", "http://xircles.codehaus.org/lists/dev@geb.codehaus.org", "<a href=\"http://xircles.codehaus.org/lists/dev@geb.codehaus.org\">http://xircles.codehaus.org/lists/dev@geb.codehaus.org</a>"),
    ("escaped inline qualified url using macro syntax should not create link", "\\http://asciidoc.org[AsciiDoc] is the key to good docs.", "http://asciidoc.org[AsciiDoc] is the key to good docs."),
    ("inline qualified url followed by a newline should not include newline in link", "The source code for Asciidoctor can be found at https://github.com/asciidoctor\nwhich is a GitHub organization.", "The source code for Asciidoctor can be found at <a href=\"https://github.com/asciidoctor\">https://github.com/asciidoctor</a> which is a GitHub organization."),
    ("qualified url divided by newline using macro syntax should not create link", "The source code for Asciidoctor can be found at link:https://github.com/asciidoctor\n[]which is a GitHub organization.", "The source code for Asciidoctor can be found at link:https://github.com/asciidoctor []which is a GitHub organization."),
    ("qualified url containing whitespace using macro syntax should not create link", "I often need to refer to the chapter on link:http://asciidoc.org?q=attribute references[Attribute References].", "I often need to refer to the chapter on link:http://asciidoc.org?q=attribute references[Attribute References]."),
    ("qualified url containing an encoded space using macro syntax should create a link", "I often need to refer to the chapter on link:http://asciidoc.org?q=attribute%20references[Attribute References].", "I often need to refer to the chapter on <a href=\"http://asciidoc.org?q=attribute%20references\">Attribute References</a>."),
    ("inline quoted qualified url should not consume surrounding angled brackets", "Asciidoctor GitHub organization: <**https://github.com/asciidoctor**>", "Asciidoctor GitHub organization: <<strong><a href=\"https://github.com/asciidoctor\">https://github.com/asciidoctor</a></strong>>"),
    ("link with quoted text should not be separated into attributes when text contains an equal sign", "http://search.example.com[\"Google, Yahoo, Bing = Search Engines\"]", "<a href=\"http://search.example.com\">Google, Yahoo, Bing = Search Engines</a>"),
    ("should leave link text as is if it contains an equals sign but no attributes are found", "https://example.com[What You Need\n= What You Get]", "<a href=\"https://example.com\">What You Need = What You Get</a>"),
    ("link with quoted text but no equal sign should carry quotes over to output", "http://search.example.com[\"Google, Yahoo, Bing\"]", "<a href=\"http://search.example.com\">\"Google, Yahoo, Bing\"</a>"),
    ("link with comma in text but no equal sign should not be separated into attributes", "http://search.example.com[Google, Yahoo, Bing]", "<a href=\"http://search.example.com\">Google, Yahoo, Bing</a>"),
    ("should process role and window attributes on link", "http://google.com[Google, role=external, window=\"_blank\"]", "<a href=\"http://google.com\" class=\"external\">Google</a>"),
    ("should parse link with wrapped text that includes attributes", "https://example.com[Foo\nBar,role=foobar]", "<a href=\"https://example.com\" class=\"foobar\">Foo Bar</a>"),
    ("link macro with attributes but no text should use URL as text", "link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[family=Roboto,weight=400]", "<a href=\"https://fonts.googleapis.com/css?family=Roboto:400,400italic,\">https://fonts.googleapis.com/css?family=Roboto:400,400italic,</a>"),
    ("link macro with attributes but blank text should use URL as text", "link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[,family=Roboto,weight=400]", "<a href=\"https://fonts.googleapis.com/css?family=Roboto:400,400italic,\">https://fonts.googleapis.com/css?family=Roboto:400,400italic,</a>"),
    ("link macro with comma but no explicit attributes in text should not parse text", "link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[Roboto,400]", "<a href=\"https://fonts.googleapis.com/css?family=Roboto:400,400italic,\">Roboto,400</a>"),
    ("link macro should support id and role attributes", "link:https://fonts.googleapis.com/css?family=Roboto:400[,id=roboto-regular,role=font]", "<a href=\"https://fonts.googleapis.com/css?family=Roboto:400\" class=\"font\">https://fonts.googleapis.com/css?family=Roboto:400</a>"),
    ("rel=noopener should be added to a link that targets a named window when the noopener option is set", "http://google.com[Google,window=name,opts=noopener]", "<a href=\"http://google.com\">Google</a>"),
    ("rel=noopener should not be added to a link if it does not target a window", "http://google.com[Google,opts=noopener]", "<a href=\"http://google.com\">Google</a>"),
    ("rel=nofollow should be added to a link when the nofollow option is set", "http://google.com[Google,window=name,opts=\"nofollow,noopener\"]", "<a href=\"http://google.com\">Google</a>"),
    ("id attribute on link is processed", "http://google.com[Google, id=\"link-1\"]", "<a href=\"http://google.com\">Google</a>"),
    ("title attribute on link is processed", "http://google.com[Google, title=\"title-1\"]", "<a href=\"http://google.com\">Google</a>"),
    ("inline irc link", "irc://irc.freenode.net", "<a href=\"irc://irc.freenode.net\">irc://irc.freenode.net</a>"),
    ("inline irc link with text", "irc://irc.freenode.net[Freenode IRC]", "<a href=\"irc://irc.freenode.net\">Freenode IRC</a>"),
    ("inline ref cannot start with digit", "[[1-install]] text", "[[1-install]] text"),
    ("xref macro with implicit inter-document target should preserve path with file extension 3", "xref:sections.d/first[First Section]", "<a href=\"#sections.d/first\">First Section</a>"),
    ("xref macro target containing dot should be interpreted as a path unless prefixed by # 2", "xref:#using-.net-web-services[Using .NET web services]", "<a href=\"#using-.net-web-services\">Using .NET web services</a>"),
];

/// 本家と答えが違う物。(名前, 入力, 本家, うち, 違う理由)
const DIFFERS: &[(&str, &str, &str, &str, &str)] = &[
    ("link with formatted wrapped text should not be separated into attributes", "https://example.com[[.role]#Foo\nBar#]", "<a href=\"https://example.com\"><span class=\"role\">Foo Bar</span></a>", "<a href=\"https://example.com\">[.role]#Foo Bar#</a>", "リンクの字の中の書式は模型に無い(run 1つに書式1つ)。元の形の字として残し、本家に組ませれば同じになる"),
    ("link text that ends in ^ should set link window to _blank", "http://google.com[Google^]", "<a href=\"http://google.com\">Google</a>", "<a href=\"http://google.com\">Google^</a>", "別の窓で開く印 `^` は模型に無い。字のまま残す"),
    ("rel=noopener should be added to a link that targets the _blank window", "http://google.com[Google^]", "<a href=\"http://google.com\">Google</a>", "<a href=\"http://google.com\">Google^</a>", "別の窓で開く印 `^` は模型に無い。字のまま残す"),
    ("inline ref can start with colon", "[[:idname]] text", "<a></a> text", "[[:idname]] text", "行の中のアンカーは模型に無い(段落のしおりは行頭の `[[名前]]` だけ)"),
    ("repeating inline anchor macro with empty reftext", "anchor:one[] anchor:two[] anchor:three[]", "<a></a> <a></a> <a></a>", "anchor:one[] anchor:two[] anchor:three[]", "行の中のアンカーは模型に無い(段落のしおりは行頭の `[[名前]]` だけ)"),
    ("mixed inline anchor macro and anchor shorthand with empty reftext", "anchor:one[][[two]]anchor:three[][[four]]anchor:five[]", "<a></a><a></a><a></a><a></a><a></a>", "anchor:one[][[two]]anchor:three[][[four]]anchor:five[]", "行の中のアンカーは模型に無い(段落のしおりは行頭の `[[名前]]` だけ)"),
    ("inter-document xref shorthand syntax should assume AsciiDoc extension if AsciiDoc extension not present", "<<using-.net-web-services#,Using .NET web services>>", "<a href=\"using-.net-web-services.html\">Using .NET web services</a>", "<a href=\"#using-.net-web-services#\">Using .NET web services</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("inter-document xref shorthand syntax should assume AsciiDoc extension if AsciiDoc extension not present 2", "<<asciidoctor.1#,Asciidoctor Manual>>", "<a href=\"asciidoctor.1.html\">Asciidoctor Manual</a>", "<a href=\"#asciidoctor.1#\">Asciidoctor Manual</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("inter-document xref shorthand syntax should assume AsciiDoc extension if AsciiDoc extension not present 3", "<<path/to/document#,Document Title>>", "<a href=\"path/to/document.html\">Document Title</a>", "<a href=\"#path/to/document#\">Document Title</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present", "xref:using-.net-web-services#[Using .NET web services]", "<a href=\"using-.net-web-services\">Using .NET web services</a>", "<a href=\"#using-.net-web-services#\">Using .NET web services</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present 2", "xref:asciidoctor.1#[Asciidoctor Manual]", "<a href=\"asciidoctor.1\">Asciidoctor Manual</a>", "<a href=\"#asciidoctor.1#\">Asciidoctor Manual</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present 3", "xref:document#[Document Title]", "<a href=\"document.html\">Document Title</a>", "<a href=\"#document#\">Document Title</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present 4", "xref:path/to/document#[Document Title]", "<a href=\"path/to/document.html\">Document Title</a>", "<a href=\"#path/to/document#\">Document Title</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present 5", "xref:include.d/document#[Document Title]", "<a href=\"include.d/document.html\">Document Title</a>", "<a href=\"#include.d/document#\">Document Title</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with implicit inter-document target should preserve path with file extension", "xref:refcard.pdf[Refcard]", "<a href=\"refcard.pdf\">Refcard</a>", "<a href=\"#refcard.pdf\">Refcard</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro with implicit inter-document target should preserve path with file extension 2", "xref:asciidoctor.1[Asciidoctor Manual]", "<a href=\"asciidoctor.1\">Asciidoctor Manual</a>", "<a href=\"#asciidoctor.1\">Asciidoctor Manual</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("inter-document xref should only remove the file extension part if the path contains a period elsewhere", "<<using-.net-web-services.adoc#,Using .NET web services>>", "<a href=\"using-.net-web-services.html\">Using .NET web services</a>", "<a href=\"#using-.net-web-services.adoc#\">Using .NET web services</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("xref macro target containing dot should be interpreted as a path unless prefixed by #", "xref:using-.net-web-services[Using .NET web services]", "<a href=\"using-.net-web-services\">Using .NET web services</a>", "<a href=\"#using-.net-web-services\">Using .NET web services</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("should not interpret double underscore in target of xref macro if sequence is preceded by a backslash", "xref:doc\\__with_double__underscore.adoc[text]", "<a href=\"doc__with_double__underscore.html\">text</a>", "<a href=\"#doc__with_double__underscore.adoc\">text</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("should not interpret double underscore in target of xref shorthand if sequence is preceded by a backslash", "<<doc\\__with_double__underscore.adoc#,text>>", "<a href=\"doc__with_double__underscore.html\">text</a>", "<a href=\"#doc__with_double__underscore.adoc#\">text</a>", "文書をまたぐ参照。しおりの名前としてそのまま持ち、別の文書への行き先は解かない"),
    ("should not match numeric character references in path of interdocument xref", "see xref:{cpp}[{cpp}].\n", "see <a href=\"#C++\">C++</a>.", "see <a href=\"#{cpp}\">{cpp}</a>.", "属性の参照(`{名前}`)は別の回で扱う"),
];

/// 写さなかった物。(名前, 理由)
#[allow(dead_code)]
const SKIPPED: &[(&str, &str)] = &[
    ("should remove trailing space on reftext of inline anchor shorthand", "文書の構造(節・複数の段落)が要る"),
    ("unescapes square bracket in reftext of anchor macro", "文書の構造(節・複数の段落)が要る"),
    ("xref using angled bracket syntax with label", "文書の構造(節・複数の段落)が要る"),
    ("xref should use title of target as link text when no explicit reftext is specified", "文書の構造(節・複数の段落)が要る"),
    ("xref should use title of target as link text when explicit link text is empty", "文書の構造(節・複数の段落)が要る"),
    ("xref using angled bracket syntax with quoted label", "文書の構造(節・複数の段落)が要る"),
    ("xref using angled bracket syntax inline with text", "文書の構造(節・複数の段落)が要る"),
    ("xref using angled bracket syntax with multi-line label inline with text", "文書の構造(節・複数の段落)が要る"),
    ("xref with escaped text", "文書の構造(節・複数の段落)が要る"),
    ("xref with target that begins with attribute reference in title", "文書の構造(節・複数の段落)が要る"),
    ("xref with target that begins with attribute reference in title 2", "文書の構造(節・複数の段落)が要る"),
    ("multiple xref macros with implicit text in single line", "文書の構造(節・複数の段落)が要る"),
    ("xref using macro syntax with label", "文書の構造(節・複数の段落)が要る"),
    ("xref using macro syntax inline with text", "文書の構造(節・複数の段落)が要る"),
    ("xref using macro syntax with multi-line label inline with text", "文書の構造(節・複数の段落)が要る"),
    ("xref using macro syntax with text that ends with an escaped closing bracket", "文書の構造(節・複数の段落)が要る"),
    ("xref using macro syntax with text that contains an escaped closing bracket", "文書の構造(節・複数の段落)が要る"),
    ("unescapes square bracket in reftext used by xref", "文書の構造(節・複数の段落)が要る"),
    ("should warn and create link if verbose flag is set and reference is not found", "文書の構造(節・複数の段落)が要る"),
    ("should warn and create link if verbose flag is set and reference using # notation is not found", "文書の構造(節・複数の段落)が要る"),
    ("xref uses title of target as label for forward and backward references in html output", "文書の構造(節・複数の段落)が要る"),
    ("should not fail to resolve broken xref in title of block with ID", "文書の構造(節・複数の段落)が要る"),
    ("should resolve forward xref in title of block with ID", "文書の構造(節・複数の段落)が要る"),
    ("should not fail to resolve broken xref in section title", "文書の構造(節・複数の段落)が要る"),
    ("should break circular xref reference in section title", "文書の構造(節・複数の段落)が要る"),
    ("should drop nested anchor in xreftext", "文書の構造(節・複数の段落)が要る"),
    ("should not resolve forward xref evaluated during parsing", "文書の構造(節・複数の段落)が要る"),
    ("should not resolve forward natural xref evaluated during parsing", "文書の構造(節・複数の段落)が要る"),
    ("should resolve first matching natural xref", "文書の構造(節・複数の段落)が要る"),
    ("should not match numeric character references while searching for fragment in xref target", "文書の構造(節・複数の段落)が要る"),
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
