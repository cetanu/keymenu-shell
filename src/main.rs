mod config;
mod menu;
mod shell;
mod ui;

use std::{
    env,
    path::PathBuf,
    process::{Command, ExitCode},
};

use anyhow::{Context, Result};
use usage::{Args, Cli, Subcommands};

use crate::{config::Config, menu::Menu};

/// Mnemonic key chords for your shell.
#[derive(Cli)]
#[usage(
    bin = "keymenu",
    version,
    usage = "keymenu [--config PATH]\nkeymenu select [--config PATH]\nkeymenu shell <fish|bash|zsh>",
    after_help = "Config: $KEYMENU_CONFIG, or $XDG_CONFIG_HOME/keymenu/config.keymenu,\n        or $HOME/.config/keymenu/config.keymenu"
)]
struct Arguments {
    /// Use this configuration file.
    #[usage(short = 'c', long, global, value_name = "PATH", value_hint = usage::ValueHint::FilePath)]
    config: Option<PathBuf>,

    /// Optional upper bound for menu description display width.
    #[usage(long, global, value_name = "COLUMNS")]
    max_description_width: Option<usize>,

    #[usage(subcommand)]
    command: Option<Subcommand>,
}

#[derive(Subcommands)]
enum Subcommand {
    /// Select a command and write it to standard output.
    Select(Select),
    /// Print shell integration code.
    Shell(Shell),
}

#[derive(Args)]
struct Select;

#[derive(Args)]
struct Shell {
    /// The shell to generate integration for.
    #[usage(choices("fish", "bash", "zsh"))]
    shell: String,
}

fn choose(
    config_path: Option<PathBuf>,
    max_description_width: Option<usize>,
) -> Result<Option<String>> {
    let path = config_path.unwrap_or(config::default_path()?);
    let config =
        Config::load(&path).with_context(|| format!("could not load {}", path.display()))?;
    let menu = Menu::new(config)?;
    ui::choose(&menu, max_description_width)
}

fn execute_selected(
    config_path: Option<PathBuf>,
    max_description_width: Option<usize>,
) -> Result<ExitCode> {
    let Some(command) = choose(config_path, max_description_width)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let shell = env::var_os("SHELL").context("SHELL is not set")?;
    let status = Command::new(&shell)
        .arg("-c")
        .arg(&command)
        .status()
        .with_context(|| format!("failed to execute shell {shell:?}"))?;
    let code = status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1);
    Ok(ExitCode::from(code))
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let result = match arguments.command {
        None => execute_selected(arguments.config, arguments.max_description_width),
        Some(Subcommand::Select(Select)) => {
            choose(arguments.config, arguments.max_description_width).map(|command| {
                if let Some(command) = command {
                    println!("{command}");
                }
                ExitCode::SUCCESS
            })
        }
        Some(Subcommand::Shell(Shell { shell })) => shell::integration(&shell).map(|integration| {
            print!("{integration}");
            ExitCode::SUCCESS
        }),
    };

    match result {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("keymenu: {error:#}");
            ExitCode::FAILURE
        }
    }
}
