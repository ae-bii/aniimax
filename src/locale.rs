//! Display-only translations. Solver identifiers and saved configuration stay in English.

use clap::ValueEnum;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Language {
    #[default]
    En,
    Ru,
    #[value(name = "zh-tw")]
    ZhTw,
}

type Catalog = HashMap<String, String>;

/// Text that clap prints itself. The catalogs do not hold these phrases.
pub struct CliText {
    pub usage: &'static str,
    pub options: &'static str,
    pub help: &'static str,
    pub version: &'static str,
    /// Pairs to replace in the long help, in this order.
    pub help_terms: &'static [(&'static str, &'static str)],
    /// Pairs to replace in clap errors, in this order. Put longer phrases first.
    pub error_terms: &'static [(&'static str, &'static str)],
}

const RUSSIAN_CLI: CliText = CliText {
    usage: "Использование:",
    options: "Параметры:",
    help: "Показать справку",
    version: "Показать версию",
    help_terms: &[
        ("[OPTIONS]", "[ПАРАМЕТРЫ]"),
        ("[default:", "[по умолчанию:"),
        ("[possible values:", "[доступные значения:"),
    ],
    error_terms: &[
        ("error:", "ошибка:"),
        ("the following required arguments were not provided:", "не указаны обязательные аргументы:"),
        ("a value is required for", "для параметра требуется значение"),
        ("invalid value", "недопустимое значение"),
        (" for '", " для '"),
        ("invalid float literal", "некорректное число"),
        ("invalid digit found in string", "некорректная цифра в числе"),
        ("value must be in range", "значение должно быть в диапазоне"),
        (" is not in ", " не входит в диапазон "),
        ("unexpected argument", "неизвестный параметр"),
        (" found", " обнаружен"),
        ("Usage:", "Использование:"),
        ("For more information, try '--help'.", "Подробнее: '--help'."),
        ("[OPTIONS]", "[ПАРАМЕТРЫ]"),
    ],
};

const TRADITIONAL_CHINESE_CLI: CliText = CliText {
    usage: "用法:",
    options: "選項:",
    help: "顯示說明",
    version: "顯示版本",
    help_terms: &[
        ("[OPTIONS]", "[選項]"),
        ("[default:", "[預設:"),
        ("[possible values:", "[可用值:"),
    ],
    error_terms: &[
        ("error:", "錯誤:"),
        ("the following required arguments were not provided:", "缺少下列必要參數:"),
        ("a value is required for", "必須提供值:"),
        ("invalid value", "無效的值"),
        (" for '", "，參數 '"),
        ("invalid float literal", "不是有效的數字"),
        ("invalid digit found in string", "數字中有無效字元"),
        ("value must be in range", "值必須在範圍內"),
        (" is not in ", " 不在範圍 "),
        ("unexpected argument", "無法識別的參數"),
        (" found", ""),
        ("Usage:", "用法:"),
        ("For more information, try '--help'.", "更多資訊請執行 '--help'。"),
        ("[OPTIONS]", "[選項]"),
    ],
};

/// Gives the number of terminal columns that `text` takes. A CJK character takes 2 columns.
pub fn display_width(text: &str) -> usize {
    text.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6)
}

/// Adds spaces after `text` until it takes `width` columns. `{:<width}` counts characters, not columns.
pub fn pad_end(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(display_width(text))))
}

/// Adds spaces before `text` until it takes `width` columns.
pub fn pad_start(text: &str, width: usize) -> String {
    format!("{}{text}", " ".repeat(width.saturating_sub(display_width(text))))
}

impl Language {
    /// Reads a `--language` value. An unknown or missing value gives English.
    pub fn parse(code: Option<&str>) -> Self {
        code.and_then(|code| Self::from_str(code, true).ok()).unwrap_or_default()
    }

    fn catalog(self) -> Option<&'static Catalog> {
        static RUSSIAN: OnceLock<Catalog> = OnceLock::new();
        static TRADITIONAL_CHINESE: OnceLock<Catalog> = OnceLock::new();
        match self {
            Self::En => None,
            Self::Ru => Some(RUSSIAN.get_or_init(|| {
                serde_json::from_str(include_str!("../web/locales/ru.json")).expect("valid Russian catalog")
            })),
            Self::ZhTw => Some(TRADITIONAL_CHINESE.get_or_init(|| {
                serde_json::from_str(include_str!("../web/locales/zh-TW.json"))
                    .expect("valid Traditional Chinese catalog")
            })),
        }
    }

    /// Gives the clap text for this language. English uses the clap defaults.
    pub fn cli_text(self) -> Option<&'static CliText> {
        match self {
            Self::En => None,
            Self::Ru => Some(&RUSSIAN_CLI),
            Self::ZhTw => Some(&TRADITIONAL_CHINESE_CLI),
        }
    }

    /// Gives the time units for hours, minutes and seconds.
    pub fn time_units(self) -> [&'static str; 3] {
        match self {
            Self::En => ["h", "m", "s"],
            Self::Ru => ["ч", "м", "с"],
            Self::ZhTw => [" 小時", " 分", " 秒"],
        }
    }

    pub fn text<'a>(self, english: &'a str) -> &'a str {
        self.catalog()
            .and_then(|catalog| catalog.get(english))
            .map(String::as_str)
            .unwrap_or(english)
    }

    pub fn item(self, english: &str) -> String {
        if self == Self::En {
            return english.to_owned();
        }
        if let Some(item) = english.strip_suffix(" (for energy)") {
            return format!("{} {}", self.item(item), self.text("(for energy)"));
        }
        let name = english.split('_').map(|word| {
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
                None => String::new(),
            }
        }).collect::<Vec<_>>().join(" ");
        self.text(&name).to_owned()
    }

    pub fn facility(self, english: &str) -> String {
        if self == Self::En { return english.to_owned(); }
        match english.split_once(" (") {
            Some((name, detail)) => format!("{} ({detail}", self.text(name)),
            None => self.text(english).to_owned(),
        }
    }
}
