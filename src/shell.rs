use anyhow::{Result, bail};

pub fn integration(shell: &str) -> Result<&'static str> {
    match shell {
        "fish" => Ok(r#"function K --description 'Open keymenu'
    set -l keymenu_command (command keymenu select)
    and test -n "$keymenu_command"
    and eval "$keymenu_command"
end
"#),
        "bash" | "zsh" => Ok(r#"K() {
    local keymenu_command
    keymenu_command="$(command keymenu select)" &&
        [[ -z "$keymenu_command" ]] || eval "$keymenu_command"
}
"#),
        _ => bail!("unsupported shell {shell:?}; expected fish, bash, or zsh"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_documented_shells() {
        for shell in ["fish", "bash", "zsh"] {
            assert!(
                integration(shell)
                    .unwrap()
                    .contains("command keymenu select")
            );
        }
    }
}
