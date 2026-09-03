use std::collections::BTreeMap;

use anyhow::{Result, bail};

use crate::config::{Binding, Config, Group};

const OMITTED: &str = "…";
const BROKEN_COMMAND: &str = "[missing command]";

#[derive(Debug, Default)]
struct Node {
    description: Option<String>,
    command: Option<String>,
    children: BTreeMap<char, Node>,
}

#[derive(Clone, Copy, Debug)]
pub struct Choice<'a> {
    pub key: char,
    pub description: &'a str,
    pub is_group: bool,
}

#[derive(Debug)]
pub struct Menu {
    root: Node,
}

impl Menu {
    pub fn new(config: Config) -> Result<Self> {
        if config.bindings.is_empty() {
            bail!("config must contain at least one keybind");
        }

        let mut root = Node::default();
        for binding in config.bindings {
            insert_binding(&mut root, binding)?;
        }
        for group in config.groups {
            insert_group(&mut root, group)?;
        }
        validate(&root, &mut String::new())?;
        Ok(Self { root })
    }

    pub fn choices(&self, prefix: &[char]) -> Vec<Choice<'_>> {
        let Some(node) = self.node(prefix) else {
            return Vec::new();
        };
        node.children
            .iter()
            .map(|(&key, child)| Choice {
                key,
                description: child.description.as_deref().unwrap_or(OMITTED),
                is_group: !child.children.is_empty(),
            })
            .collect()
    }

    pub fn command(&self, keys: &[char]) -> Option<&str> {
        self.node(keys)?.command.as_deref()
    }

    pub fn contains_prefix(&self, keys: &[char]) -> bool {
        self.node(keys).is_some()
    }

    pub fn is_broken(&self, keys: &[char]) -> bool {
        self.node(keys)
            .is_some_and(|node| node.command.is_none() && node.children.is_empty())
    }

    fn node(&self, keys: &[char]) -> Option<&Node> {
        let mut node = &self.root;
        for key in keys {
            node = node.children.get(key)?;
        }
        Some(node)
    }
}

fn key_path<'a>(root: &'a mut Node, keys: &str, kind: &str) -> Result<&'a mut Node> {
    validate_keys(keys, kind)?;
    let mut node = root;
    for key in keys.chars() {
        node = node.children.entry(key).or_default();
    }
    Ok(node)
}

fn validate_keys(keys: &str, kind: &str) -> Result<()> {
    if keys.is_empty() {
        bail!("{kind} keys cannot be empty");
    }
    for key in keys.chars() {
        if key.is_control() {
            bail!("{kind} {keys:?} contains a control character");
        }
    }
    Ok(())
}

fn insert_group(root: &mut Node, group: Group) -> Result<()> {
    if group
        .description
        .as_ref()
        .is_some_and(|description| description.trim().is_empty())
    {
        bail!("group {:?} has an empty description", group.keys);
    }
    validate_keys(&group.keys, "group")?;
    let Some(node) = node_at_path_mut(root, &group.keys) else {
        return Ok(());
    };
    if node.command.is_some() {
        return Ok(());
    }
    if node
        .description
        .replace(group.description.unwrap_or_else(|| OMITTED.into()))
        .is_some()
    {
        bail!("duplicate group {:?}", group.keys);
    }
    Ok(())
}

fn node_at_path_mut<'a>(root: &'a mut Node, keys: &str) -> Option<&'a mut Node> {
    let mut node = root;
    for key in keys.chars() {
        node = node.children.get_mut(&key)?;
    }
    Some(node)
}

fn insert_binding(root: &mut Node, binding: Binding) -> Result<()> {
    if binding
        .description
        .as_ref()
        .is_some_and(|description| description.trim().is_empty())
    {
        bail!("binding {:?} has an empty description", binding.keys);
    }
    if binding
        .command
        .as_deref()
        .is_some_and(|command| command.trim().is_empty())
    {
        bail!("binding {:?} has an empty command", binding.keys);
    }
    let keys = binding.keys.clone();
    let node = key_path(root, &keys, "binding")?;
    match node.command {
        Some(_) => bail!("duplicate binding {keys:?}"),
        None => {
            let description = match (&binding.description, &binding.command) {
                (Some(description), Some(_)) => description.clone(),
                (Some(description), None) => format!("{description} {BROKEN_COMMAND}"),
                (None, Some(command)) => command.clone(),
                (None, None) => BROKEN_COMMAND.into(),
            };
            node.description = Some(description);
            node.command = binding.command;
            Ok(())
        }
    }
}

fn validate(node: &Node, prefix: &mut String) -> Result<()> {
    if node.command.is_some() && !node.children.is_empty() {
        bail!("binding {prefix:?} is also a prefix of another binding");
    }
    for (key, child) in &node.children {
        prefix.push(*key);
        validate(child, prefix)?;
        prefix.pop();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            groups: vec![Group {
                keys: "g".into(),
                description: Some("Git".into()),
            }],
            bindings: vec![
                Binding {
                    keys: "gs".into(),
                    description: Some("Status".into()),
                    command: Some("git status".into()),
                },
                Binding {
                    keys: "f".into(),
                    description: Some("Files".into()),
                    command: Some("ls".into()),
                },
            ],
        }
    }

    #[test]
    fn builds_nested_choices_and_resolves_commands() {
        let menu = Menu::new(config()).unwrap();
        let root = menu.choices(&[]);
        assert_eq!(
            root.iter().map(|item| item.key).collect::<Vec<_>>(),
            ['f', 'g']
        );
        assert!(root[1].is_group);
        assert_eq!(menu.choices(&['g'])[0].description, "Status");
        assert_eq!(menu.command(&['g', 's']), Some("git status"));
    }

    #[test]
    fn rejects_ambiguous_binding_prefixes() {
        let mut config = config();
        config.bindings.push(Binding {
            keys: "g".into(),
            description: Some("Git command".into()),
            command: Some("git".into()),
        });
        let error = Menu::new(config).unwrap_err();
        assert!(error.to_string().contains("also a prefix"));
    }

    #[test]
    fn ignores_groups_without_bindings() {
        let mut config = config();
        config.groups.push(Group {
            keys: "x".into(),
            description: Some("Unused".into()),
        });
        let menu = Menu::new(config).unwrap();
        assert!(menu.choices(&[]).iter().all(|choice| choice.key != 'x'));
    }

    #[test]
    fn uses_command_as_description_when_omitted() {
        let config = Config {
            groups: vec![],
            bindings: vec![Binding {
                keys: "s".into(),
                description: None,
                command: Some("git status".into()),
            }],
        };
        let menu = Menu::new(config).unwrap();
        assert_eq!(menu.choices(&[])[0].description, "git status");
    }

    #[test]
    fn marks_bindings_without_commands_as_broken() {
        let config = Config {
            groups: vec![],
            bindings: vec![Binding {
                keys: "z".into(),
                description: Some("Broken shortcut".into()),
                command: None,
            }],
        };
        let menu = Menu::new(config).unwrap();

        assert_eq!(
            menu.choices(&[])[0].description,
            "Broken shortcut [missing command]"
        );
        assert!(menu.is_broken(&['z']));
        assert_eq!(menu.command(&['z']), None);
    }

    #[test]
    fn uses_ellipsis_for_groups_without_descriptions() {
        let config = Config {
            groups: vec![Group {
                keys: "g".into(),
                description: None,
            }],
            bindings: vec![Binding {
                keys: "gs".into(),
                description: None,
                command: Some("git status".into()),
            }],
        };
        let menu = Menu::new(config).unwrap();
        assert_eq!(menu.choices(&[])[0].description, OMITTED);
    }
}
