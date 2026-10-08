//! Tests for the Traditional Chinese (zh-TW) display text of the CLI.

use aniimax::locale::{zh_tw, Language};
use clap::ValueEnum;

#[test]
fn test_zh_tw_spellings_parse() {
    for code in zh_tw::CODES {
        assert_eq!(Language::from_str(code, false), Ok(Language::ZhTw), "{code}");
        assert!(zh_tw::is_code(code), "{code}");
    }
    assert!(!zh_tw::is_code("zh"));
}

#[test]
fn test_zh_tw_text() {
    assert_eq!(Language::ZhTw.item("moonray_wheat"), "月芒穗");
    assert_eq!(Language::ZhTw.facility("Farmland"), "田地");
    assert_eq!(Language::ZhTw.facility("Farmland (x2)"), "田地 (x2)");
}

/// English text that the zh-TW catalog does not hold, for example a phrase added later, stays in English.
#[test]
fn test_zh_tw_falls_back_to_english() {
    assert_eq!(Language::ZhTw.text("A phrase added after the zh-TW catalog"), "A phrase added after the zh-TW catalog");
    assert_eq!(Language::ZhTw.item("new_item_name"), "New Item Name");
    assert_eq!(Language::ZhTw.facility("New Facility (x2)"), "New Facility (x2)");
}

#[test]
fn test_zh_tw_duration() {
    assert_eq!(zh_tw::format_duration(3665.0), "1 小時 1 分 5 秒");
    assert_eq!(zh_tw::format_duration(45.0), "45 秒");
}

#[test]
fn test_zh_tw_padding_counts_columns() {
    assert_eq!(zh_tw::display_width("田地 x2"), 7);
    assert_eq!(zh_tw::pad_end("田地", 6), "田地  ");
    assert_eq!(zh_tw::pad_start("田地", 6), "  田地");
    assert_eq!(zh_tw::pad_end("田地田地", 2), "田地田地");
}

#[test]
fn test_zh_tw_help_and_error_terms() {
    assert_eq!(zh_tw::help_text("aniimax [OPTIONS] [default: 1]".into()), "aniimax [選項] [預設: 1]");
    assert!(zh_tw::error_text("error: unexpected argument '--x' found".into()).starts_with("錯誤: 無法識別的參數 '--x'"));
}
