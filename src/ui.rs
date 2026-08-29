use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, ClearType},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::menu::Menu;

pub fn choose(menu: &Menu, max_description_width: Option<usize>) -> Result<Option<String>> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("interactive use requires stdin and stderr to be terminals");
    }

    let _raw_mode = RawMode::enter()?;
    let mut stderr = io::stderr().lock();
    let mut prefix = Vec::new();
    let mut rendered_lines = 0;

    loop {
        draw(
            &mut stderr,
            menu,
            &prefix,
            rendered_lines,
            max_description_width,
        )?;
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

fn draw(
    out: &mut impl Write,
    menu: &Menu,
    prefix: &[char],
    old_lines: usize,
    max_description_width: Option<usize>,
) -> Result<()> {
    clear(out, old_lines)?;
    let choices = menu.choices(prefix);
    let key_width = choices
        .iter()
        .map(|choice| choice.key.width().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let arrow_width = if choices.iter().any(|choice| choice.is_group) {
        3
    } else {
        0
    };
    let terminal_width = terminal::size()?.0 as usize;
    let available_width = terminal_width.saturating_sub(key_width + 2 + arrow_width);
    let description_limit =
        max_description_width.map_or(available_width, |limit| limit.min(available_width));
    let descriptions: Vec<String> = choices
        .iter()
        .map(|choice| truncate_description(choice.description, description_limit))
        .collect();

    queue!(out, Print(" "), SetAttribute(Attribute::Bold))?;
    if prefix.is_empty() {
        queue!(out, Print("keymenu"))?;
    } else {
        for (index, key) in prefix.iter().enumerate() {
            queue!(
                out,
                SetForegroundColor(Color::White),
                SetBackgroundColor(Color::DarkGrey),
                Print(" "),
                Print(key),
                Print(" "),
                ResetColor,
                SetAttribute(Attribute::Bold),
                Print(if index + 1 < prefix.len() {
                    " → "
                } else {
                    ""
                }),
                SetAttribute(Attribute::Reset)
            )?;
        }
    }
    queue!(
        out,
        SetAttribute(Attribute::Reset),
        ResetColor,
        Print("  "),
        SetForegroundColor(Color::DarkGrey),
        Print("Esc cancel · ⌫ back\r\n"),
        ResetColor
    )?;

    for (choice, description) in choices.into_iter().zip(descriptions) {
        queue!(
            out,
            Print("  "),
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(choice.key),
            SetAttribute(Attribute::Reset),
            ResetColor,
            SetForegroundColor(if choice.is_group {
                Color::DarkGrey
            } else {
                Color::DarkMagenta
            }),
            Print(if choice.is_group { " ↳ " } else { " ▸ " }),
            SetAttribute(if choice.is_group {
                Attribute::Bold
            } else {
                Attribute::Reset
            }),
            ResetColor,
            Print(format!("{}", description)),
            Print("\r\n")
        )?;
    }
    out.flush()?;
    Ok(())
}

fn truncate_description(description: &str, max_width: usize) -> String {
    if description.width() <= max_width {
        return description.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }

    let ellipsis = '…';
    let content_width = max_width.saturating_sub(ellipsis.width().unwrap_or(1));
    let mut result = String::new();
    let mut width = 0;
    for character in description.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width > content_width {
            break;
        }
        result.push(character);
        width += character_width;
    }
    result.push(ellipsis);
    result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_descriptions_to_display_width() {
        let description = truncate_description("A very long description", 10);

        assert_eq!(description, "A very lo…");
        assert_eq!(description.width(), 10);
    }

    #[test]
    fn does_not_split_wide_characters() {
        let description = truncate_description("界界界", 5);

        assert_eq!(description, "界界…");
        assert_eq!(description.width(), 5);
    }
}
