use std::{fs, process::Command};

#[test]
fn hotkey_runs_in_shell_and_preserves_prompt_status() {
    check_prompt("zsh");
}

#[test]
fn fish_hotkey_runs_in_shell_and_preserves_prompt_status() {
    check_prompt("fish");
}

fn check_prompt(shell: &str) {
    if Command::new("expect").arg("-v").output().is_err()
        || Command::new(shell).arg("--version").output().is_err()
    {
        eprintln!("skipping interactive test: expect or zsh unavailable");
        return;
    }

    let config = std::env::temp_dir().join(format!(
        "keymenu-test-{shell}-{}.config",
        std::process::id()
    ));
    fs::write(&config, "keybind(\"f\", \"return 7\")\n").unwrap();
    let binary = env!("CARGO_BIN_EXE_keymenu");
    let binary_dir = std::path::Path::new(binary).parent().unwrap();
    let path = std::env::join_paths(std::iter::once(binary_dir.to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    let output = Command::new("expect")
        .arg(format!("tests/{shell}_prompt.expect"))
        .arg(binary)
        .arg(&config)
        .env("PATH", path)
        .output()
        .unwrap();
    fs::remove_file(config).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
