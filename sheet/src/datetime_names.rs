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
    },
    Names {
        lang: "de",
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_abbr: ["Jan", "Feb", "Mrz", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
        months_genitive: None,
        days: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
        days_abbr: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
        long_date: "dddd, d. mmmm yyyy",
    },
    Names {
        lang: "en",
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_abbr: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        months_genitive: None,
        days: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
        days_abbr: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        long_date: "dddd, mmmm d, yyyy",
    },
    Names {
        lang: "es",
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_abbr: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sep.", "oct.", "nov.", "dic."],
        months_genitive: None,
        days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
        days_abbr: ["do.", "lu.", "ma.", "mi.", "ju.", "vi.", "sá."],
        long_date: "dddd, d\" de \"mmmm\" de \"yyyy",
    },
    Names {
        lang: "fr",
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_abbr: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        months_genitive: None,
        days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
        days_abbr: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        long_date: "dddd d mmmm yyyy",
    },
    Names {
        lang: "id",
        months: ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"],
        months_abbr: ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"],
        months_genitive: None,
        days: ["Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu"],
        days_abbr: ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"],
        long_date: "dddd, dd mmmm yyyy",
    },
    Names {
        lang: "it",
        months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
        months_abbr: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
        months_genitive: None,
        days: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
        days_abbr: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
        long_date: "dddd d mmmm yyyy",
    },
    Names {
        lang: "ko",
        months: ["1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월"],
        months_abbr: ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"],
        months_genitive: None,
        days: ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"],
        days_abbr: ["일", "월", "화", "수", "목", "금", "토"],
        long_date: "yyyy\"년\" m\"월\" d\"일\" dddd",
    },
    Names {
        lang: "pt",
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_abbr: ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
        months_genitive: None,
        days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        days_abbr: ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
        long_date: "d\" de \"mmmm\" de \"yyyy",
    },
    Names {
        lang: "ru",
        months: ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"],
        months_abbr: ["янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"],
        months_genitive: Some(["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"]),
        days: ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"],
        days_abbr: ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"],
        long_date: "d mmmm yyyy \"г.\"",
    },
    Names {
        lang: "tr",
        months: ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"],
        months_abbr: ["Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"],
        months_genitive: None,
        days: ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"],
        days_abbr: ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"],
        long_date: "d mmmm yyyy dddd",
    },
    Names {
        lang: "vi",
        months: ["Tháng Giêng", "Tháng Hai", "Tháng Ba", "Tháng Tư", "Tháng Năm", "Tháng Sáu", "Tháng Bảy", "Tháng Tám", "Tháng Chín", "Tháng Mười", "Tháng Mười Một", "Tháng Mười Hai"],
        months_abbr: ["Thg1", "Thg2", "Thg3", "Thg4", "Thg5", "Thg6", "Thg7", "Thg8", "Thg9", "Thg10", "Thg11", "Thg12"],
        months_genitive: None,
        days: ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
        days_abbr: ["CN", "T2", "T3", "T4", "T5", "T6", "T7"],
        long_date: "dd mmmm yyyy",
    },
    Names {
        lang: "zh",
        months: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_abbr: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
        months_genitive: None,
        days: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
        days_abbr: ["周日", "周一", "周二", "周三", "周四", "周五", "周六"],
        long_date: "yyyy\"年\"m\"月\"d\"日\"",
    },
    Names {
        lang: "zh-tw",
        months: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_abbr: ["一月", "二月", "三月", "四月", "五月", "六月", "七月", "八月", "九月", "十月", "十一月", "十二月"],
        months_genitive: None,
        days: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
        days_abbr: ["週日", "週一", "週二", "週三", "週四", "週五", "週六"],
        long_date: "yyyy\"年\"m\"月\"d\"日\"",
    },
];

/// その言語の暦の語。**知らない言語は日本語に落ちる** — 素の言語だから。
/// 黙って英語にすると、日本語で使っている人に英語が出る事故になる
pub fn names(lang: &str) -> &'static Names {
    TABLE
        .iter()
        .find(|n| n.lang == lang)
        // "zh-tw" のような枝が無ければ "zh" へ、それも無ければ ja
        .or_else(|| lang.split_once('-').and_then(|(base, _)| {
            TABLE.iter().find(|n| n.lang == base)
        }))
        .unwrap_or(&TABLE[0])
}
