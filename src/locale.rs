//! Display-only translations. Solver identifiers and saved configuration stay in English.

use clap::ValueEnum;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Language {
    #[default]
    En,
    Ru,
}

impl Language {
    pub fn text<'a>(self, english: &'a str) -> &'a str {
        if self == Self::En {
            return english;
        }
        static RUSSIAN: OnceLock<HashMap<String, String>> = OnceLock::new();
        RUSSIAN
            .get_or_init(|| serde_json::from_str(include_str!("../web/locales/ru.json")).expect("valid Russian catalog"))
            .get(english)
            .map(String::as_str)
            .unwrap_or(english)
    }

    pub fn item(self, english: &str) -> String {
        if self == Self::En {
            return english.to_owned();
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
