//! Reading and changing a project's `composer.json` without disturbing the
//! rest of it: keys stay in order and the indentation is kept.

use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use serde_json::{Map, Value};

/// The command Composer runs to prefix, which `init` adds and `migrate` points
/// people to.
pub const PREFIX_COMMAND: &str = "sh bin/strauss-rs.sh prefix";

const SCRIPT_NAME: &str = "prefix-namespaces";
const HOOKS: [&str; 2] = ["post-install-cmd", "post-update-cmd"];

pub struct ComposerFile {
    value: Value,
    indent: String,
    trailing_newline: bool,
}

impl ComposerFile {
    pub fn parse(text: &str) -> Result<ComposerFile, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;

        if !value.is_object() {
            return Err("must contain a JSON object".to_string());
        }

        Ok(ComposerFile {
            value,
            indent: detect_indent(text),
            trailing_newline: text.ends_with('\n'),
        })
    }

    /// `None` if there is no `composer.json`.
    pub fn read(path: &Path) -> Result<Option<(ComposerFile, String)>, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("could not read {}: {error}", path.display())),
        };

        let file = ComposerFile::parse(&text).map_err(|error| format!("composer.json: {error}"))?;

        Ok(Some((file, text)))
    }

    pub fn render(&self) -> String {
        let mut output = Vec::new();
        let formatter = PrettyFormatter::with_indent(self.indent.as_bytes());
        let mut serializer = serde_json::Serializer::with_formatter(&mut output, formatter);

        self.value
            .serialize(&mut serializer)
            .expect("a JSON value serialises");

        let mut text = String::from_utf8(output).expect("JSON is UTF-8");

        if self.trailing_newline {
            text.push('\n');
        }

        text
    }

    pub fn extra(&self, key: &str) -> Option<&Value> {
        self.value.get("extra")?.get(key)
    }

    /// Removes `extra.strauss`, and `extra` too if nothing else is in it.
    pub fn remove_extra_strauss(&mut self) -> bool {
        let Some(root) = self.value.as_object_mut() else {
            return false;
        };
        let Some(extra) = root.get_mut("extra").and_then(Value::as_object_mut) else {
            return false;
        };

        let removed = extra.shift_remove("strauss").is_some();

        if removed && extra.is_empty() {
            root.shift_remove("extra");
        }

        removed
    }

    /// Adds a `prefix-namespaces` script, and runs it after `composer install`
    /// and `composer update`. Returns what changed, for the user.
    pub fn add_prefix_scripts(&mut self) -> Vec<String> {
        let mut changes = Vec::new();
        let root = self.value.as_object_mut().expect("checked when parsed");

        if !root.get("scripts").is_some_and(Value::is_object) {
            root.insert("scripts".to_string(), Value::Object(Map::new()));
        }

        let scripts = root
            .get_mut("scripts")
            .and_then(Value::as_object_mut)
            .expect("just made an object");

        if !scripts.contains_key(SCRIPT_NAME) {
            scripts.insert(
                SCRIPT_NAME.to_string(),
                Value::Array(vec![Value::String(PREFIX_COMMAND.to_string())]),
            );
            changes.push(format!("added the `{SCRIPT_NAME}` script"));
        }

        let reference = format!("@{SCRIPT_NAME}");

        for hook in HOOKS {
            let mut commands: Vec<Value> = match scripts.get(hook) {
                None => Vec::new(),
                Some(Value::String(command)) => vec![Value::String(command.clone())],
                Some(Value::Array(commands)) => commands.clone(),
                // Not something Composer accepts; leave it alone.
                Some(_) => continue,
            };

            if commands.iter().any(|command| command == &reference) {
                continue;
            }

            commands.push(Value::String(reference.clone()));
            scripts.insert(hook.to_string(), Value::Array(commands));
            changes.push(format!("run `{reference}` from `{hook}`"));
        }

        changes
    }

    /// Scripts that run PHP Strauss, as `(script, command)`. Those that run
    /// strauss-rs already aren't included.
    pub fn php_strauss_commands(&self) -> Vec<(String, String)> {
        let Some(scripts) = self.value.get("scripts").and_then(Value::as_object) else {
            return Vec::new();
        };

        let mut found = Vec::new();

        for (name, value) in scripts {
            let commands: Vec<&str> = match value {
                Value::String(command) => vec![command.as_str()],
                Value::Array(commands) => commands.iter().filter_map(Value::as_str).collect(),
                _ => continue,
            };

            for command in commands {
                if command.contains("strauss") && !command.contains("strauss-rs") {
                    found.push((name.clone(), command.to_string()));
                }
            }
        }

        found
    }
}

/// The whitespace in front of the first indented line, as Composer would have
/// written it: four spaces if nothing says otherwise.
fn detect_indent(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim_start_matches([' ', '\t']);
            let indent = &line[..line.len() - trimmed.len()];

            (!indent.is_empty() && trimmed.starts_with('"')).then(|| indent.to_string())
        })
        .unwrap_or_else(|| "    ".to_string())
}

/// `strauss.json` as the commands write it: two spaces, as in the README.
pub fn render_strauss_json(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("a JSON value serialises");

    text.push('\n');

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ComposerFile {
        ComposerFile::parse(text).unwrap()
    }

    #[test]
    fn keeps_the_order_and_indentation_it_was_given() {
        let text = "{\n\t\"name\": \"a/b\",\n\t\"require\": {\n\t\t\"z/z\": \"1\",\n\t\t\"a/a\": \"2\"\n\t}\n}\n";

        assert_eq!(parse(text).render(), text);
    }

    #[test]
    fn indents_with_four_spaces_when_nothing_says_otherwise() {
        assert_eq!(
            parse("{\"name\":\"a/b\"}").render(),
            "{\n    \"name\": \"a/b\"\n}"
        );
    }

    #[test]
    fn follows_the_files_trailing_newline() {
        assert!(parse("{\"a\":1}\n").render().ends_with('\n'));
        assert!(!parse("{\"a\":1}").render().ends_with('\n'));
    }

    #[test]
    fn rejects_something_that_is_not_an_object() {
        assert!(ComposerFile::parse("[]").is_err());
        assert!(ComposerFile::parse("{").is_err());
    }

    #[test]
    fn removes_extra_strauss_and_an_extra_left_empty() {
        let mut file = parse(r#"{"extra": {"strauss": {"a": 1}, "other": true}}"#);

        assert!(file.remove_extra_strauss());
        assert!(file.extra("other").is_some());
        assert!(file.extra("strauss").is_none());

        let mut file = parse(r#"{"name": "a/b", "extra": {"strauss": {}}}"#);

        assert!(file.remove_extra_strauss());
        assert!(!file.render().contains("extra"));
        assert!(!file.remove_extra_strauss());
    }

    #[test]
    fn adds_the_scripts_once() {
        let mut file = parse(r#"{"scripts": {"post-install-cmd": "echo hi"}}"#);

        assert_eq!(file.add_prefix_scripts().len(), 3);
        assert!(file.add_prefix_scripts().is_empty());

        let value: Value = serde_json::from_str(&file.render()).unwrap();

        assert_eq!(
            value["scripts"]["post-install-cmd"],
            serde_json::json!(["echo hi", "@prefix-namespaces"])
        );
        assert_eq!(
            value["scripts"]["post-update-cmd"],
            serde_json::json!(["@prefix-namespaces"])
        );
        assert_eq!(
            value["scripts"]["prefix-namespaces"],
            serde_json::json!([PREFIX_COMMAND])
        );
    }

    #[test]
    fn keeps_a_prefix_script_that_is_already_there() {
        let mut file = parse(r#"{"scripts": {"prefix-namespaces": "custom"}}"#);

        file.add_prefix_scripts();

        assert!(file.render().contains("\"prefix-namespaces\": \"custom\""));
    }

    #[test]
    fn finds_the_scripts_that_run_php_strauss() {
        let file = parse(
            r#"{"scripts": {
                "a": "vendor/bin/strauss",
                "b": ["@php x.php", "vendor/bin/strauss --foo"],
                "c": "sh bin/strauss-rs.sh prefix"
            }}"#,
        );

        assert_eq!(
            file.php_strauss_commands(),
            [
                ("a".to_string(), "vendor/bin/strauss".to_string()),
                ("b".to_string(), "vendor/bin/strauss --foo".to_string())
            ]
        );
    }
}
