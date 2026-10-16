//! Reads `config.toml` and turns its `[keybindings]` sections into
//! [`KeyBindings`].

use serde::Deserialize;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Action, Context, KeyBind, KeyBindings, UserBindings};

/// A fully commented configuration file listing every action at its default
/// key, printed by `--print-default-config`.
pub const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("default_config.toml");

const CONFIG_FILE: &str = "systemd-manager-tui/config.toml";

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    keybindings: toml::Table,
}

/// The configuration file read when `--config` is not given:
/// `$XDG_CONFIG_HOME/systemd-manager-tui/config.toml`, falling back to
/// `~/.config/systemd-manager-tui/config.toml`.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(Path::new(&config_home).join(CONFIG_FILE));
    }

    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| Path::new(&home).join(".config").join(CONFIG_FILE))
}

/// Loads the key map. A missing default configuration file is not an error; a
/// missing `--config` file is.
pub fn load(explicit_path: Option<&Path>) -> Result<KeyBindings, String> {
    let (path, required) = match explicit_path {
        Some(path) => (path.to_path_buf(), true),
        None => match default_config_path() {
            Some(path) => (path, false),
            None => return Ok(KeyBindings::default()),
        },
    };

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound && !required => {
            return Ok(KeyBindings::default());
        }
        Err(error) => {
            return Err(format!(
                "Could not read the configuration file {}: {error}",
                path.display()
            ));
        }
    };

    parse(&text).map_err(|error| format!("Invalid configuration in {}\n{error}", path.display()))
}

/// Parses configuration text into a resolved key map.
pub fn parse(text: &str) -> Result<KeyBindings, String> {
    let file: ConfigFile = toml::from_str(text).map_err(|error| error.to_string())?;
    let user = collect(&file.keybindings)?;
    KeyBindings::resolve(&user)
}

fn collect(table: &toml::Table) -> Result<UserBindings, String> {
    let mut user = UserBindings::default();

    for (name, value) in table {
        if let toml::Value::Table(section) = value {
            let context = Context::VIEWS
                .into_iter()
                .find(|context| context.key() == name)
                .ok_or_else(|| {
                    format!(
                        "unknown section `[keybindings.{name}]` (expected one of {})",
                        section_names()
                    )
                })?;

            for (action_name, action_value) in section {
                collect_entry(context, action_name, action_value, &mut user)?;
            }
        } else {
            collect_entry(Context::Global, name, value, &mut user)?;
        }
    }

    Ok(user)
}

fn collect_entry(
    context: Context,
    name: &str,
    value: &toml::Value,
    user: &mut UserBindings,
) -> Result<(), String> {
    let specs = key_specs(context, name, value)?;
    let keys = specs
        .iter()
        .map(|spec| {
            KeyBind::parse(spec)
                .map_err(|error| format!("[{}]: `{name}`: {error}", context.section()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if name == "unbind" {
        for key in keys {
            user.unbind(context, key);
        }
        return Ok(());
    }

    let action =
        Action::from_name(context, name).ok_or_else(|| unknown_action_error(context, name))?;
    user.bind(context, action, keys);

    Ok(())
}

/// A binding value is either a single key or a list of keys.
fn key_specs(context: Context, name: &str, value: &toml::Value) -> Result<Vec<String>, String> {
    let type_error = || {
        format!(
            "[{}]: `{name}` must be a key such as \"ctrl+c\", or a list of keys such as \
             [\"down\", \"j\"]",
            context.section()
        )
    };

    match value {
        toml::Value::String(spec) => Ok(vec![spec.clone()]),
        toml::Value::Array(specs) => specs
            .iter()
            .map(|spec| spec.as_str().map(str::to_string).ok_or_else(type_error))
            .collect(),
        _ => Err(type_error()),
    }
}

fn unknown_action_error(context: Context, name: &str) -> String {
    let misplaced = [
        Context::Global,
        Context::List,
        Context::Log,
        Context::UnitFile,
    ]
    .into_iter()
    .find(|other| *other != context && Action::from_name(*other, name).is_some());

    if let Some(other) = misplaced {
        return format!(
            "[{}]: `{name}` is not an action here; it belongs in [{}]",
            context.section(),
            other.section()
        );
    }

    format!(
        "[{}]: unknown action `{name}`. Valid actions here: {}, unbind",
        context.section(),
        action_names(context)
    )
}

fn action_names(context: Context) -> String {
    context
        .defaults()
        .iter()
        .map(|(action, _)| action.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn section_names() -> String {
    Context::VIEWS
        .into_iter()
        .map(Context::key)
        .collect::<Vec<_>>()
        .join(", ")
}
