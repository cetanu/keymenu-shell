# keymenu

`keymenu` brings mnemonic key chords to the terminal. Open the menu, see the
available keys, and type a chord such as `g` then `s` to run `git status`.

Running `keymenu` directly executes the selected command using `$SHELL`. The
optional shell integration evaluates it in the current shell, which also lets
state-changing commands such as `cd` and `export` affect your active session.

## Install

Build and install the binary with Rust:

```console
cargo install --path .
```

Create `~/.config/keymenu/config.keymenu` (or set `KEYMENU_CONFIG` to another
path):

```text
group("g", description: "Git")

keybind("gs", description: "Status", command: "git status")
keybind("gl", description: "Recent log", command: "git log --oneline --decorate -20")
keybind("p", description: "Open projects", command: "cd ~/projects")
keybind("u", command: "uname -a")
```

The configuration language accepts only `group` and `keybind` statements.
Whitespace, argument order, trailing commas, and `#` comments are supported.
Strings support `\"`, `\\`, `\n`, `\r`, and `\t` escapes. Unknown statements,
unknown or duplicate arguments, and malformed strings are errors.

`description` is required for groups and optional for keybindings. When a
keybinding omits it, the command is displayed as its description.

The first string is the key chord. Intermediate prefixes are inferred, and a
`group` gives a prefix a useful name. A binding cannot also be the prefix of
another binding because it would be ambiguous.

## Shell setup

`keymenu` works by itself. For commands that must modify the active shell, the
generated integration defines the single-character function `K`.

For fish, add this to `~/.config/fish/config.fish`:

```fish
keymenu shell fish | source
```

For zsh, add this to `~/.zshrc`:

```zsh
eval "$(keymenu shell zsh)"
```

For bash, add this to `~/.bashrc`:

```bash
eval "$(keymenu shell bash)"
```

Then type `K` at the prompt. Press a displayed key to continue, Backspace to
move to the parent menu, or Escape/Ctrl-C to cancel.

Other shells can use the same protocol: capture `keymenu select`'s standard
output and evaluate it only when non-empty and successful. The interactive UI
and errors are written through standard error.

## Configuration lookup

The first available location is used:

1. `$KEYMENU_CONFIG`
2. `$XDG_CONFIG_HOME/keymenu/config.keymenu`
3. `$HOME/.config/keymenu/config.keymenu`

Pass `--config PATH` to select a file for one invocation. Run `keymenu --help`
for the complete command summary.

See [`config.example.keymenu`](config.example.keymenu) for a ready-to-copy example.
