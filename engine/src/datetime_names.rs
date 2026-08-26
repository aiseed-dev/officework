//! 月名・曜日名と、言語ごとの「長い日付」の既定。
//!
//! **このファイルは sheet/gen_datetime_names.py が生成する。手で書かない。**
//! 材料は vendor/sdkjs/common/NumFormat.js の cultureInfo(本家 ONLYOFFICE、
//! AGPL-3.0)。`calc/gen_funcs.py` と同じ作法で、依存は増やさない。
//!
//! **通貨記号は載せていない。** 本家の表は持っているが、通貨は読む人の
//! 言語ではなくその帳票のお金なので、言語から引ける形にしない
//! (docs/sekkei/calc.ja.md「通貨だけは言語に引かせない」)。
//!
//! 曜日は **0 が日曜**(`calc::weekday0` と `YOBI` に合わせてある)。

/// ひとつの言語の暦の語。
pub struct Names {
    pub lang: &'static str,
    pub months: [&'static str; 12],
    pub months_abbr: [&'static str; 12],
    /// 属格(「8月**の**」)。チェコ語・ロシア語・ギリシャ語・フィンランド語
    /// などは日付の中で形が変わる。持たない言語は None
    pub months_genitive: Option<[&'static str; 12]>,
    /// 0 = 日曜
    pub days: [&'static str; 7],
    pub days_abbr: [&'static str; 7],
    /// その言語の「長い日付」の既定(Excel の書式コード)。
    /// 発注者の「各国で一つに決めて置いたほうがいい」に当たる物で、
    /// **本家が決めた既定をそのまま使う** — こちらで13本を考え直さない
    pub long_date: &'static str,
    /// その言語の「短い日付」の既定(Excel の書式コード)
    pub short_date: &'static str,
    /// この言語を指す地域番号。**書式コードに `[$-407]` として入れる** —
    /// こちらが日付の書式を掛けるとき、何語で書いたかをファイルに残すため。
    /// 残さないと、開いた人の環境しだいで別の月名が出る
    pub lcid: u32,
    /// 通貨記号の**置き場所**だけ(0=記号n / 1=n記号 / 2=記号␣n / 3=n␣記号)。
    /// **記号そのものは持たない** — お金は読む人の言語ではなく帳票のもの
    /// (docs/sekkei/calc.ja.md)。並びだけが言語の作法
    pub currency_pattern: u8,
}

pub const TABLE: &[Names] = &[
    Names {
        lang: "ja",
        months: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_abbr: ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"],
        months_genitive: None,
        days: ["日曜日", "月曜日", "火曜日", "水曜日", "木曜日", "金曜日", "土曜日"],
        days_abbr: ["日", "月", "火", "水", "木", "金", "土"],
        long_date: "yyyy\"年\"m\"月\"d\"日\"",
        short_date: "yyyy/mm/dd",
        currency_pattern: 0,
        lcid: 0x411,
    },
    Names {
        lang: "de",
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_abbr: ["Jan", "Feb", "Mrz", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
        months_genitive: None,
        days: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
        days_abbr: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
        long_date: "dddd, d. mmmm yyyy",
        short_date: "dd.mm.yyyy",
        currency_pattern: 3,
        lcid: 0x407,
    },
    Names {
        lang: "en",
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_abbr: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        months_genitive: None,
        days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
        days_abbr: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        long_date: "dd mmmm yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 0,
        lcid: 0x809,
    },
    Names {
        lang: "es",
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_abbr: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
        months_genitive: None,
        days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
        days_abbr: ["do.", "lu.", "ma.", "mi.", "ju.", "vi.", "sá."],
        long_date: "dddd, d\" de \"mmmm\" de \"yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 3,
        lcid: 0xc0a,
    },
    Names {
        lang: "fr",
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_abbr: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        months_genitive: None,
        days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
        days_abbr: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        long_date: "dddd d mmmm yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 3,
        lcid: 0x40c,
    },
    Names {
        lang: "id",
        months: ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"],
        months_abbr: ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"],
        months_genitive: None,
        days: ["Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu"],
        days_abbr: ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"],
        long_date: "dddd, dd mmmm yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 0,
        lcid: 0x421,
    },
    Names {
        lang: "it",
        months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
        months_abbr: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
        months_genitive: None,
        days: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
        days_abbr: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
        long_date: "dddd d mmmm yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 3,
        lcid: 0x410,
    },
    Names {
        lang: "ko",
        months: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        months_abbr: ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"],
        months_genitive: None,
        days: ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"],
        days_abbr: ["일", "월", "화", "수", "목", "금", "토"],
        long_date: "yyyy\"년\" m\"월\" d\"일\" dddd",
        short_date: "yyyy-mm-dd",
        currency_pattern: 0,
        lcid: 0x412,
    },
    Names {
        lang: "pt",
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_abbr: ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
        months_genitive: None,
        days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        days_abbr: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
        long_date: "d\" de \"mmmm\" de \"yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 3,
        lcid: 0x816,
    },
    Names {
        lang: "pt-br",
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_abbr: ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
        months_genitive: None,
        days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        days_abbr: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
        long_date: "dddd, d\" de \"mmmm\" de \"yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 2,
        lcid: 0x416,
    },
    Names {
        lang: "ru",
        months: ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"],
        months_abbr: ["янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"],
        months_genitive: Some(["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"]),
        days: ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"],
        days_abbr: ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"],
        long_date: "d mmmm yyyy \"г.\"",
        short_date: "dd.mm.yyyy",
        currency_pattern: 3,
        lcid: 0x419,
    },
    Names {
        lang: "tr",
        months: ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"],
        months_abbr: ["Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"],
        months_genitive: None,
        days: ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"],
        days_abbr: ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"],
        long_date: "d mmmm yyyy dddd",
        short_date: "d.mm.yyyy",
        currency_pattern: 0,
        lcid: 0x41f,
    },
    Names {
        lang: "vi",
        months: ["Tháng Giêng", "Tháng Hai", "Tháng Ba", "Tháng Tư", "Tháng Năm", "Tháng Sáu", "Tháng Bảy", "Tháng Tám", "Tháng Chín", "Tháng Mười", "Tháng Mười Một", "Tháng Mười Hai"],
        months_abbr: ["Thg1", "Thg2", "Thg3", "Thg4", "Thg5", "Thg6", "Thg7", "Thg8", "Thg9", "Thg10", "Thg11", "Thg12"],
        months_genitive: None,
        days: ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
        days_abbr: ["CN", "T2", "T3", "T4", "T5", "T6", "T7"],
        long_date: "dd mmmm yyyy",
        short_date: "dd/mm/yyyy",
        currency_pattern: 3,
        lcid: 0x42a,
    },
    Names {
        lang: "zh",
        months: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_abbr: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_genitive: None,
        days: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
        days_abbr: ["周日", "周一", "周二", "周三", "周四", "周五", "周六"],
        long_date: "yyyy\"年\"m\"月\"d\"日\"",
        short_date: "yyyy/m/d",
        currency_pattern: 0,
        lcid: 0x804,
    },
    Names {
        lang: "zh-tw",
        months: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_abbr: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_genitive: None,
        days: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
        days_abbr: ["週日", "週一", "週二", "週三", "週四", "週五", "週六"],
        long_date: "yyyy\"年\"m\"月\"d\"日\"",
        short_date: "yyyy/m/d",
        currency_pattern: 0,
        lcid: 0x404,
    },
];

/// LCID → うちの言語。書式コードの `[$-407]`(独)`[$-409]`(米)から引く。
/// **本家の表から起こしてある** — 暗記だと pt の 416/816 を踏み外す
pub const LCID_LANG: &[(u32, &str)] = &[
    (0x4, "zh"),
    (0x7, "de"),
    (0x9, "en"),
    (0xa, "es"),
    (0xc, "fr"),
    (0x10, "it"),
    (0x11, "ja"),
    (0x12, "ko"),
    (0x16, "pt"),
    (0x19, "ru"),
    (0x1f, "tr"),
    (0x21, "id"),
    (0x2a, "vi"),
    (0x404, "zh-tw"),
    (0x407, "de"),
    (0x409, "en"),
    (0x40c, "fr"),
    (0x410, "it"),
    (0x411, "ja"),
    (0x412, "ko"),
    (0x416, "pt-br"),
    (0x419, "ru"),
    (0x41f, "tr"),
    (0x421, "id"),
    (0x42a, "vi"),
    (0x804, "zh"),
    (0x807, "de"),
    (0x809, "en"),
    (0x80a, "es"),
    (0x80c, "fr"),
    (0x810, "it"),
    (0x816, "pt"),
    (0x819, "ru"),
    (0xc04, "zh-tw"),
    (0xc07, "de"),
    (0xc09, "en"),
    (0xc0a, "es"),
    (0xc0c, "fr"),
    (0x1004, "zh"),
    (0x1007, "de"),
    (0x1009, "en"),
    (0x100a, "es"),
    (0x100c, "fr"),
    (0x1404, "zh-tw"),
    (0x1407, "de"),
    (0x1409, "en"),
    (0x140a, "es"),
    (0x140c, "fr"),
    (0x1809, "en"),
    (0x180a, "es"),
    (0x180c, "fr"),
    (0x1c09, "en"),
    (0x1c0a, "es"),
    (0x1c0c, "fr"),
    (0x2009, "en"),
    (0x200a, "es"),
    (0x200c, "fr"),
    (0x2409, "en"),
    (0x240a, "es"),
    (0x240c, "fr"),
    (0x2809, "en"),
    (0x280a, "es"),
    (0x280c, "fr"),
    (0x2c09, "en"),
    (0x2c0a, "es"),
    (0x2c0c, "fr"),
    (0x3009, "en"),
    (0x300a, "es"),
    (0x300c, "fr"),
    (0x3409, "en"),
    (0x340a, "es"),
    (0x340c, "fr"),
    (0x3809, "en"),
    (0x380a, "es"),
    (0x380c, "fr"),
    (0x3c09, "en"),
    (0x3c0a, "es"),
    (0x3c0c, "fr"),
    (0x4009, "en"),
    (0x400a, "es"),
    (0x4409, "en"),
    (0x440a, "es"),
    (0x4809, "en"),
    (0x480a, "es"),
    (0x4c0a, "es"),
    (0x500a, "es"),
    (0x540a, "es"),
    (0x580a, "es"),
    (0x5c0a, "es"),
    (0x7804, "zh"),
    (0x7c04, "zh-tw"),
];

/// 書式コードの地域指定から言語を引く。**知らない番号は None** —
/// 勝手に近い言語へ寄せない(寄せた先が違えば、静かに別の月名が出る)
pub fn lang_of_lcid(lcid: u32) -> Option<&'static str> {
    LCID_LANG
        .binary_search_by_key(&lcid, |(k, _)| *k)
        .ok()
        .map(|i| LCID_LANG[i].1)
}

/// その言語の暦の語。**知らない言語は日本語に落ちる** — 素の言語だから。
/// 黙って英語にすると、日本語で使っている人に英語が出る事故になる
pub fn names(lang: &str) -> &'static Names {
    TABLE
        .iter()
        .find(|n| n.lang.eq_ignore_ascii_case(lang))
        // "zh-tw" のような枝が無ければ "zh" へ、それも無ければ ja
        .or_else(|| lang.split_once('-').and_then(|(base, _)| {
            TABLE.iter().find(|n| n.lang.eq_ignore_ascii_case(base))
        }))
        .unwrap_or(&TABLE[0])
}
