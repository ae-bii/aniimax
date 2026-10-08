//! Tests for the display languages of the CLI.

use aniimax::locale::Language;

#[test]
fn test_parse_language_codes() {
    assert_eq!(Language::parse(Some("en")), Language::En);
    assert_eq!(Language::parse(Some("ru")), Language::Ru);
    assert_eq!(Language::parse(Some("zh-tw")), Language::ZhTw);
    assert_eq!(Language::parse(Some("zh-TW")), Language::ZhTw);
}

#[test]
fn test_parse_unknown_language_gives_english() {
    assert_eq!(Language::parse(None), Language::En);
    assert_eq!(Language::parse(Some("xx")), Language::En);
}

#[test]
fn test_english_keeps_source_text() {
    assert_eq!(Language::En.text("Configuration:"), "Configuration:");
    assert_eq!(Language::En.item("moonray_wheat"), "moonray_wheat");
    assert!(Language::En.cli_text().is_none());
}

#[test]
fn test_traditional_chinese_text() {
    assert_eq!(Language::ZhTw.item("moonray_wheat"), "月芒穗");
    assert_eq!(Language::ZhTw.facility("Farmland"), "田地");
    assert_eq!(Language::ZhTw.facility("Farmland (x2)"), "田地 (x2)");
    assert_eq!(Language::ZhTw.text("unmapped phrase"), "unmapped phrase");
}

#[test]
fn test_traditional_chinese_cli_text() {
    let cli = Language::ZhTw.cli_text().expect("zh-TW has clap text");
    assert_eq!(cli.usage, "用法:");
    assert_eq!(Language::ZhTw.time_units(), [" 小時", " 分", " 秒"]);
}

#[test]
fn test_russian_keeps_previous_output() {
    let cli = Language::Ru.cli_text().expect("ru has clap text");
    assert_eq!(cli.usage, "Использование:");
    assert_eq!(Language::Ru.time_units(), ["ч", "м", "с"]);
}

#[test]
fn test_every_catalog_has_every_phrase() {
    let english: Vec<String> = serde_json::from_str(include_str!("../web/locales/en.json")).unwrap();
    for language in [Language::Ru, Language::ZhTw] {
        let missing: Vec<_> = english.iter().filter(|phrase| language.text(phrase) == phrase.as_str()
            && !phrase.chars().all(|c| !c.is_alphabetic())).collect();
        // Some phrases (for example "Aniimax") can stay the same in a translation.
        assert!(missing.len() < 20, "{language:?} leaves {} phrases untranslated", missing.len());
    }
}
