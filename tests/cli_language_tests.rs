//! Tests that run the CLI binary with each spelling of the zh-TW language code.

use std::process::{Command, Output};

const ZH_TW_SPELLINGS: [&str; 3] = ["zh-tw", "zh-TW", "ZH-TW"];

/// Runs the CLI from the project root, where it finds the `data` directory.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aniimax"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the CLI starts")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn test_zh_tw_spellings_work_in_a_calculation() {
    for code in ZH_TW_SPELLINGS {
        let output = run(&["--language", code, "--target", "100"]);
        assert!(output.status.success(), "{code}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(stdout(&output).contains("設定:"), "{code} prints Chinese output");
    }
}

#[test]
fn test_zh_tw_spellings_work_in_help() {
    for code in ZH_TW_SPELLINGS {
        let output = run(&["--language", code, "--help"]);
        assert!(output.status.success(), "{code}");
        assert!(stdout(&output).contains("用法:"), "{code} prints Chinese help");
    }
}
