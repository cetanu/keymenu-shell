use std::{collections::BTreeMap, env, fs, path::Path, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};

#[derive(Debug)]
pub struct Config {
    pub groups: Vec<Group>,
    pub bindings: Vec<Binding>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub keys: String,
    pub description: Option<String>,
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

struct Arguments {
    positional: Vec<String>,
    keyword: BTreeMap<String, String>,
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
            let statement_line = self.line();
            let statement = self.identifier()?;
            self.skip_trivia();
            self.expect('(')?;
            self.skip_trivia();
            let keys = self.string()?;
            let mut arguments = self.arguments()?;

            match statement.as_str() {
                "group" => {
                    with_line(
                        reject_unknown(&arguments.keyword, &["description"], "group"),
                        statement_line,
                    )?;
                    let description = with_line(
                        optional_argument_value(
                            arguments.positional,
                            &mut arguments.keyword,
                            "description",
                            "group",
                        ),
                        statement_line,
                    )?;
                    config.groups.push(Group { keys, description });
                }
                "keybind" => {
                    with_line(
                        reject_unknown(&arguments.keyword, &["description", "command"], "keybind"),
                        statement_line,
                    )?;
                    let (command, description) =
                        with_line(keybind_arguments(arguments), statement_line)?;
                    config.bindings.push(Binding {
                        keys,
                        description,
                        command,
                    });
                }
                _ => return self.error(format!("unknown statement {statement:?}")),
            }
            self.skip_trivia();
        }

        Ok(config)
    }

    fn arguments(&mut self) -> Result<Arguments> {
        let mut positional = Vec::new();
        let mut keyword = BTreeMap::new();
        loop {
            self.skip_trivia();
            if self.consume(')') {
                return Ok(Arguments {
                    positional,
                    keyword,
                });
            }
            self.expect(',')?;
            self.skip_trivia();
            if self.consume(')') {
                return Ok(Arguments {
                    positional,
                    keyword,
                });
            }

            let start = self.position;
            let name = self.identifier();
            self.skip_trivia();
            if self.consume(':') {
                let name = name?;
                self.skip_trivia();
                let value = self.string()?;
                if keyword.insert(name.clone(), value).is_some() {
                    return self.error(format!("duplicate argument {name:?}"));
                }
            } else {
                self.position = start;
                positional.push(self.string()?);
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

    fn line(&self) -> usize {
        self.source[..self.position]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1
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

fn with_line<T>(result: Result<T>, line: usize) -> Result<T> {
    result.map_err(|error| anyhow!("line {line}: {error:#}"))
}

fn optional_argument_value(
    mut positional: Vec<String>,
    keyword: &mut BTreeMap<String, String>,
    name: &str,
    statement: &str,
) -> Result<Option<String>> {
    if positional.len() > 1 {
        bail!("{statement} has too many positional arguments");
    }
    match (positional.pop(), keyword.remove(name)) {
        (Some(_), Some(_)) => bail!("{statement} specifies argument {name:?} more than once"),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn keybind_arguments(arguments: Arguments) -> Result<(String, Option<String>)> {
    if arguments.positional.len() > 2 {
        bail!("keybind has too many positional arguments");
    }

    let mut keyword = arguments.keyword;
    let mut positional = arguments.positional.into_iter();
    let named_description = keyword.remove("description");
    let named_command = keyword.remove("command");

    let (description, command) = match (named_description, named_command) {
        (Some(description), Some(command)) => {
            if positional.next().is_some() {
                bail!("keybind specifies all arguments by keyword");
            }
            (Some(description), command)
        }
        (Some(description), None) => {
            let command = positional
                .next()
                .context("keybind is missing required argument \"command\"")?;
            (Some(description), command)
        }
        (None, Some(command)) => (positional.next(), command),
        (None, None) => match (positional.next(), positional.next()) {
            (Some(command), None) => (None, command),
            (Some(description), Some(command)) => (Some(description), command),
            (None, None) => bail!("keybind is missing required argument \"command\""),
            (None, Some(_)) => bail!("keybind has too many positional arguments"),
        },
    };

    if positional.next().is_some() {
        bail!("keybind has too many positional arguments");
    }
    Ok((command, description))
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
                group("g", "Git")
                keybind("gs", "Status", "printf \"ok\\n\"")
            "#,
        )
        .unwrap();

        assert_eq!(config.groups[0].description.as_deref(), Some("Git"));
        assert_eq!(config.bindings[0].keys, "gs");
        assert_eq!(config.bindings[0].description.as_deref(), Some("Status"));
        assert_eq!(config.bindings[0].command, r#"printf "ok\n""#);
    }

    #[test]
    fn accepts_keyword_arguments() {
        let config = parse(
            r#"
                group("g", description: "Git")
                keybind("gs", description: "Status", command: "git status")
            "#,
        )
        .unwrap();
        assert_eq!(config.groups[0].description.as_deref(), Some("Git"));
        assert_eq!(config.bindings[0].command, "git status");
    }

    #[test]
    fn accepts_mixed_arguments() {
        let config = parse(r#"keybind("g", "Does a thing", command: "echo foo")"#).unwrap();
        assert_eq!(
            config.bindings[0].description.as_deref(),
            Some("Does a thing")
        );
        assert_eq!(config.bindings[0].command, "echo foo");
    }

    #[test]
    fn parses_all_argument_permutations() {
        macro_rules! assert_statement {
            ($source:literal => group($keys:literal, $description:expr)) => {{
                let config = parse($source).unwrap();
                assert!(config.bindings.is_empty(), "{}", $source);
                assert_eq!(config.groups.len(), 1, "{}", $source);
                assert_eq!(config.groups[0].keys, $keys, "{}", $source);
                assert_eq!(
                    config.groups[0].description.as_deref(),
                    $description,
                    "{}",
                    $source
                );
            }};
            ($source:literal => keybind($keys:literal, $description:expr, $command:literal)) => {{
                let config = parse($source).unwrap();
                assert!(config.groups.is_empty(), "{}", $source);
                assert_eq!(config.bindings.len(), 1, "{}", $source);
                assert_eq!(config.bindings[0].keys, $keys, "{}", $source);
                assert_eq!(
                    config.bindings[0].description.as_deref(),
                    $description,
                    "{}",
                    $source
                );
                assert_eq!(config.bindings[0].command, $command, "{}", $source);
            }};
        }

        assert_statement!(r#"group("g", "Git")"# => group("g", Some("Git")));
        assert_statement!(r#"group("g", description: "Git")"# => group("g", Some("Git")));
        assert_statement!(r#"group("g")"# => group("g", None));

        assert_statement!(
            r#"keybind("g", "Does a thing", "echo foo")"#
                => keybind("g", Some("Does a thing"), "echo foo")
        );
        assert_statement!(
            r#"keybind("g", "Does a thing", command: "echo foo")"#
                => keybind("g", Some("Does a thing"), "echo foo")
        );
        assert_statement!(
            r#"keybind("g", description: "Does a thing", command: "echo foo")"#
                => keybind("g", Some("Does a thing"), "echo foo")
        );
        assert_statement!(r#"keybind("g", "echo foo")"# => keybind("g", None, "echo foo"));
        assert_statement!(
            r#"keybind("g", command: "echo foo")"# => keybind("g", None, "echo foo")
        );
        assert_statement!(
            r#"keybind("g", description: "Does a thing", "echo foo")"#
                => keybind("g", Some("Does a thing"), "echo foo")
        );
    }

    #[test]
    fn reports_line_and_column_for_syntax_errors() {
        let error = parse("# comment\nkeybind(\"g\" \"Git\")").unwrap_err();
        assert!(error.to_string().contains("line 2, column 13"));
    }

    #[test]
    fn reports_line_for_argument_validation_errors() {
        let error = parse("group(\"g\")\nkeybind(\"z\")").unwrap_err();
        assert_eq!(
            error.to_string(),
            "line 2: keybind is missing required argument \"command\""
        );
    }

    #[test]
    fn keybind_description_is_optional() {
        let config = parse(r#"keybind("s", "git status")"#).unwrap();
        assert_eq!(config.bindings[0].description, None);
    }

    #[test]
    fn example_config_is_valid() {
        let config = parse(include_str!("../config.example.keymenu")).unwrap();
        assert!(!config.groups.is_empty());
        assert!(!config.bindings.is_empty());
    }
}
