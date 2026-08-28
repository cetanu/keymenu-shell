use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, ClearType},
};
use unicode_width::UnicodeWidthStr;

use crate::menu::Menu;

pub fn choose(menu: &Menu) -> Result<Option<String>> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("interactive use requires stdin and stderr to be terminals");
    }

    let _raw_mode = RawMode::enter()?;
    let mut stderr = io::stderr().lock();
    let mut prefix = Vec::new();
    let mut rendered_lines = 0;

    loop {
        draw(&mut stderr, menu, &prefix, rendered_lines)?;
        rendered_lines = menu.choices(&prefix).len() + 1;

        let Event::Key(key) = event::read().context("failed to read terminal input")? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }

        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                clear(&mut stderr, rendered_lines)?;
                return Ok(None);
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                prefix.pop();
            }
            KeyEvent {
                code: KeyCode::Char(key),
                modifiers,
                ..
            } if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
                prefix.push(key);
                if let Some(command) = menu.command(&prefix) {
                    let command = command.to_owned();
                    clear(&mut stderr, rendered_lines)?;
                    return Ok(Some(command));
                }
                if !menu.contains_prefix(&prefix) {
                    prefix.pop();
                    write!(stderr, "\x07")?;
                    stderr.flush()?;
                }
            }
            _ => {}
        }
    }
}

fn draw(out: &mut impl Write, menu: &Menu, prefix: &[char], old_lines: usize) -> Result<()> {
    clear(out, old_lines)?;
    let choices = menu.choices(prefix);
    let chord: String = prefix.iter().collect();
    let title = if chord.is_empty() { "keymenu" } else { &chord };
    let description_width = choices
        .iter()
        .map(|choice| choice.description.width())
        .max()
        .unwrap_or(0);

    queue!(
        out,
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset),
        Print("  "),
        SetForegroundColor(Color::DarkGrey),
        Print("Esc cancel · Backspace parent\r\n"),
        ResetColor
    )?;
    for choice in choices {
        queue!(
            out,
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(choice.key),
            SetAttribute(Attribute::Reset),
            ResetColor,
            Print("  "),
            Print(format!("{:description_width$}", choice.description)),
            Print(if choice.is_group { "  →" } else { "" }),
            Print("\r\n")
        )?;
    }
    out.flush()?;
    Ok(())
}

fn clear(out: &mut impl Write, lines: usize) -> Result<()> {
    if lines == 0 {
        return Ok(());
    }
    queue!(out, cursor::MoveUp(lines as u16))?;
    for line in 0..lines {
        queue!(
            out,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine)
        )?;
        if line + 1 < lines {
            queue!(out, cursor::MoveDown(1))?;
        }
    }
    queue!(
        out,
        cursor::MoveUp((lines - 1) as u16),
        cursor::MoveToColumn(0)
    )?;
    out.flush()?;
    Ok(())
}

struct RawMode;

impl RawMode {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode().context("failed to enable terminal raw mode")?;
        if let Err(error) = execute!(io::stderr(), cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error).context("failed to hide cursor");
        }
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            io::stderr(),
            cursor::Show,
            ResetColor,
            SetAttribute(Attribute::Reset)
        );
    }
}
