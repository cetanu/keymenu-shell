use std::{collections::BTreeMap, env, fs, path::Path, path::PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug)]
pub struct Config {
    pub groups: Vec<Group>,
    pub bindings: Vec<Binding>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub keys: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub keys: String,
    pub description: Option<String>,
    pub command: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path).context("failed to read config")?;
        parse(&source)
    }
}

pub fn parse(source: &str) -> Result<Config> {
    Parser::new(source).parse()
}

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("KEYMENU_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    let config_home = match env::var_os("XDG_CONFIG_HOME") {
        Some(path) => PathBuf::from(path),
        None => {
            let home = env::var_os("HOME")
                .context("HOME is not set; set KEYMENU_CONFIG to the path of your config")?;
            PathBuf::from(home).join(".config")
        }
    };

    let path = config_home.join("keymenu/config.keymenu");
    if path.as_os_str().is_empty() {
        bail!("config path is empty");
    }
    Ok(path)
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    fn parse(mut self) -> Result<Config> {
        let mut config = Config {
            groups: Vec::new(),
            bindings: Vec::new(),
        };

        self.skip_trivia();
        while self.peek().is_some() {
            let statement = self.identifier()?;
            self.skip_trivia();
            self.expect('(')?;
            self.skip_trivia();
            let keys = self.string()?;
            let arguments = self.arguments()?;

            match statement.as_str() {
                "group" => {
                    let mut arguments = arguments;
                    reject_unknown(&arguments, &["description"], "group")?;
                    config.groups.push(Group {
                        keys,
                        description: take_argument(&mut arguments, "description", "group")?,
                    });
                }
                "keybind" => {
                    let mut arguments = arguments;
                    reject_unknown(&arguments, &["description", "command"], "keybind")?;
                    config.bindings.push(Binding {
                        keys,
                        description: arguments.remove("description"),
                        command: take_argument(&mut arguments, "command", "keybind")?,
                    });
                }
                _ => return self.error(format!("unknown statement {statement:?}")),
            }
            self.skip_trivia();
        }

        Ok(config)
    }

    fn arguments(&mut self) -> Result<BTreeMap<String, String>> {
        let mut arguments = BTreeMap::new();
        loop {
            self.skip_trivia();
            if self.consume(')') {
                return Ok(arguments);
            }
            self.expect(',')?;
            self.skip_trivia();
            if self.consume(')') {
                return Ok(arguments);
            }

            let name = self.identifier()?;
            self.skip_trivia();
            self.expect(':')?;
            self.skip_trivia();
            let value = self.string()?;
            if arguments.insert(name.clone(), value).is_some() {
                return self.error(format!("duplicate argument {name:?}"));
            }
        }
    }

    fn identifier(&mut self) -> Result<String> {
        let mut identifier = String::new();
        while let Some(character) = self.peek() {
            if character.is_ascii_alphanumeric() || character == '_' {
                identifier.push(character);
                self.advance();
            } else {
                break;
            }
        }
        if identifier.is_empty() {
            self.error("expected an identifier")
        } else {
            Ok(identifier)
        }
    }

    fn string(&mut self) -> Result<String> {
        self.expect('"')?;
        let mut value = String::new();
        loop {
            let Some(character) = self.advance() else {
                return self.error("unterminated string");
            };
            match character {
                '"' => return Ok(value),
                '\n' | '\r' => return self.error("strings cannot contain literal newlines"),
                '\\' => {
                    let Some(escaped) = self.advance() else {
                        return self.error("unterminated escape sequence");
                    };
                    value.push(match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => return self.error(format!("unsupported escape \\{escaped}")),
                    });
                }
                _ => value.push(character),
            }
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.advance();
            }
            if self.peek() != Some('#') {
                break;
            }
            while self.peek().is_some_and(|character| character != '\n') {
                self.advance();
            }
        }
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        if self.consume(expected) {
            Ok(())
        } else {
            self.error(format!("expected {expected:?}"))
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn error<T>(&self, message: impl std::fmt::Display) -> Result<T> {
        let before = &self.source[..self.position];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before
            .rsplit_once('\n')
            .map_or(before, |(_, tail)| tail)
            .chars()
            .count()
            + 1;
        bail!("line {line}, column {column}: {message}")
    }
}

fn take_argument(
    arguments: &mut BTreeMap<String, String>,
    name: &str,
    statement: &str,
) -> Result<String> {
    arguments
        .remove(name)
        .with_context(|| format!("{statement} is missing required argument {name:?}"))
}

fn reject_unknown(
    arguments: &BTreeMap<String, String>,
    allowed: &[&str],
    statement: &str,
) -> Result<()> {
    if let Some(name) = arguments
        .keys()
        .find(|name| !allowed.contains(&name.as_str()))
    {
        bail!("{statement} has unknown argument {name:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups_bindings_comments_and_escapes() {
        let config = parse(
            r#"
                # Git commands
                group("g", description: "Git")
                keybind("gs", command: "printf \"ok\\n\"", description: "Status")
            "#,
        )
        .unwrap();

        assert_eq!(config.groups[0].description, "Git");
        assert_eq!(config.bindings[0].keys, "gs");
        assert_eq!(config.bindings[0].description.as_deref(), Some("Status"));
        assert_eq!(config.bindings[0].command, r#"printf "ok\n""#);
    }

    #[test]
    fn rejects_unknown_arguments() {
        let error = parse(r#"group("g", label: "Git")"#).unwrap_err();
        assert!(error.to_string().contains("unknown argument \"label\""));
    }

    #[test]
    fn reports_line_and_column_for_syntax_errors() {
        let error = parse("# comment\nkeybind(\"g\" description: \"Git\")").unwrap_err();
        assert!(error.to_string().contains("line 2, column 13"));
    }

    #[test]
    fn keybind_description_is_optional() {
        let config = parse(r#"keybind("s", command: "git status")"#).unwrap();
        assert_eq!(config.bindings[0].description, None);
    }

    #[test]
    fn group_description_is_required() {
        let error = parse(r#"group("g")"#).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing required argument \"description\"")
        );
    }

    #[test]
    fn example_config_is_valid() {
        let config = parse(include_str!("../config.example.keymenu")).unwrap();
        assert!(!config.groups.is_empty());
        assert!(!config.bindings.is_empty());
    }
}
