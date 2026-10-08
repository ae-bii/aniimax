//! Traditional Chinese (zh-TW) display text for the CLI.
//!
//! Everything for zh-TW is here, so the code for the other languages stays as it is. A phrase that
//! the catalog does not hold, for example English text added after this catalog, shows in English.

use clap::{Arg, ArgAction, Command};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::display::format_time;
use crate::models::ProductionEfficiency;

/// The spellings of the zh-TW code that `--language` accepts. The web app writes it as zh-TW.
/// They must match the clap aliases on `Language::ZhTw`.
pub const CODES: [&str; 3] = ["zh-tw", "zh-TW", "ZH-TW"];

/// Pairs to replace in the long help, in this order.
const HELP_TERMS: [(&str, &str); 3] = [
    ("[OPTIONS]", "[選項]"),
    ("[default:", "[預設:"),
    ("[possible values:", "[可用值:"),
];

/// Pairs to replace in clap errors, in this order. Longer phrases come first.
const ERROR_TERMS: [(&str, &str); 14] = [
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
];

/// Tells if `code` is one of the zh-TW spellings in `CODES`.
pub fn is_code(code: &str) -> bool {
    CODES.contains(&code)
}

/// Gives the zh-TW text for an English phrase, or the English phrase when the catalog has none.
pub fn text(english: &str) -> &str {
    static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();
    CATALOG
        .get_or_init(|| serde_json::from_str(include_str!("../../web/locales/zh-TW.json")).expect("valid zh-TW catalog"))
        .get(english)
        .map(String::as_str)
        .unwrap_or(english)
}

/// Gives a duration with zh-TW units, for example "2 小時 4 分 3 秒".
pub fn format_duration(seconds: f64) -> String {
    format_time(seconds).replace('h', " 小時").replace('m', " 分").replace('s', " 秒")
}

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

/// Puts zh-TW text in the clap command: the about text, the help layout and each argument's help.
pub fn localize_command(command: Command) -> Command {
    let mut command = command
        .about(text("Optimize production paths for currency generation in Aniimo Homeland").to_owned())
        .help_template("{about-with-newline}\n用法: {usage}\n\n選項:\n{options}")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(Arg::new("help").short('h').long("help").action(ArgAction::Help).help("顯示說明"))
        .arg(Arg::new("version").short('V').long("version").action(ArgAction::Version).help("顯示版本"));
    let helps: Vec<_> = command
        .get_arguments()
        .filter_map(|arg| arg.get_help().map(|help| (arg.get_id().clone(), help.to_string())))
        .collect();
    for (id, help) in helps {
        command = command.mut_arg(id, |arg| arg.help(text(&help).to_owned()));
    }
    command
}

/// Puts zh-TW words in the long help that clap rendered.
pub fn help_text(rendered: String) -> String {
    replace_terms(rendered, &HELP_TERMS)
}

/// Puts zh-TW words in a clap error.
pub fn error_text(error: String) -> String {
    replace_terms(error, &ERROR_TERMS)
}

fn replace_terms(text: String, terms: &[(&str, &str)]) -> String {
    terms.iter().fold(text, |text, (english, translated)| text.replace(english, translated))
}

/// Gives an item name in zh-TW, from its id such as `quick_wheat`.
fn item(english: &str) -> String {
    crate::locale::Language::ZhTw.item(english)
}

/// Prints the ranked options table, with columns that line up for CJK text.
pub fn print_all_options(efficiencies: &[ProductionEfficiency], optimize_energy: bool) {
    println!();
    println!(
        "{} ({} {})",
        text("[ALL OPTIONS RANKED]"),
        text("by"),
        if optimize_energy { text("energy efficiency") } else { text("time efficiency") }
    );
    println!("----------------------------------------------------------------");
    println!(
        "{} {} {} {}",
        pad_end(text("Item"), 20),
        pad_start(text("Profit/sec"), 12),
        pad_start(text("Profit/energy"), 12),
        pad_start(text("Time/unit"), 12)
    );
    println!("----------------------------------------------------------------");

    let mut sorted = efficiencies.to_vec();
    if optimize_energy {
        sorted.sort_by(|a, b| {
            b.profit_per_energy
                .unwrap_or(0.0)
                .partial_cmp(&a.profit_per_energy.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        sorted.sort_by(|a, b| {
            b.profit_per_second
                .partial_cmp(&a.profit_per_second)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    for eff in sorted.iter().take(10) {
        let energy = eff.profit_per_energy.map(|e| format!("{:.4}", e)).unwrap_or_else(|| text("N/A").to_string());
        println!(
            "{} {:>12.4} {} {}",
            pad_end(&item(&eff.item.name), 20),
            eff.profit_per_second,
            pad_start(&energy, 12),
            pad_start(&format_duration(eff.total_time_per_unit), 12)
        );
    }

    println!();
}

/// Prints the energy rankings table, with columns that line up for CJK text.
pub fn print_energy_rankings(items_with_energy: &[&ProductionEfficiency]) {
    println!();
    println!("{}", text("[ENERGY EFFICIENCY RANKINGS]"));
    println!("----------------------------------------------------------------");
    println!(
        "{} {} {}",
        pad_end(text("Item"), 20),
        pad_start(text("Profit/Energy"), 15),
        pad_start(text("Energy/Unit"), 15)
    );
    println!("----------------------------------------------------------------");

    let mut sorted = items_with_energy.to_vec();
    sorted.sort_by(|a, b| {
        b.profit_per_energy
            .unwrap_or(0.0)
            .partial_cmp(&a.profit_per_energy.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for eff in sorted.iter().take(10) {
        println!(
            "{} {:>15.6} {:>15.0}",
            pad_end(&item(&eff.item.name), 20),
            eff.profit_per_energy.unwrap_or(0.0),
            eff.total_energy_per_unit.unwrap_or(0.0)
        );
    }
}
