use anyhow::{Result, bail};

const FISH_INTEGRATION: &str = r#"function K --description 'Open keymenu'
    set -l keymenu_command (command keymenu select)
    and test -n "$keymenu_command"
    and eval "$keymenu_command"
end

function __keymenu_hotkey
    if test -z (commandline)
        K
    else
        commandline --insert K
    end
end

bind K __keymenu_hotkey
"#;

const BASH_INTEGRATION: &str = r#"K() {
    local keymenu_command
    keymenu_command="$(command keymenu select)" &&
        [[ -z "$keymenu_command" ]] || eval "$keymenu_command"
}

__keymenu_hotkey() {
    if [[ -z "$READLINE_LINE" ]]; then
        K
    else
        READLINE_LINE="${READLINE_LINE:0:READLINE_POINT}K${READLINE_LINE:READLINE_POINT}"
        ((READLINE_POINT++))
    fi
}

bind -x '"K":__keymenu_hotkey'
"#;

const ZSH_INTEGRATION: &str = r#"K() {
    local keymenu_command
    keymenu_command="$(command keymenu select)" &&
        [[ -z "$keymenu_command" ]] || eval "$keymenu_command"
}

__keymenu_hotkey() {
    if [[ -z "$BUFFER" ]]; then
        zle -I
        K
        zle reset-prompt
    else
        LBUFFER+='K'
    fi
}

zle -N __keymenu_hotkey
bindkey 'K' __keymenu_hotkey
"#;

pub fn integration(shell: &str) -> Result<&'static str> {
    match shell {
        "fish" => Ok(FISH_INTEGRATION),
        "bash" => Ok(BASH_INTEGRATION),
        "zsh" => Ok(ZSH_INTEGRATION),
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

    #[test]
    fn shells_bind_uppercase_k_without_breaking_command_editing() {
        for (shell, buffer, insert, binding) in [
            (
                "fish",
                "if test -z (commandline)",
                "commandline --insert K",
                "bind K __keymenu_hotkey",
            ),
            (
                "bash",
                "if [[ -z \"$READLINE_LINE\" ]]",
                "READLINE_LINE=",
                "bind -x '\"K\":__keymenu_hotkey'",
            ),
            (
                "zsh",
                "if [[ -z \"$BUFFER\" ]]",
                "LBUFFER+='K'",
                "bindkey 'K' __keymenu_hotkey",
            ),
        ] {
            let integration = integration(shell).unwrap();
            assert!(integration.contains(buffer));
            assert!(integration.contains(insert));
            assert!(integration.contains(binding));
        }
    }
}
