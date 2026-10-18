//! Keybindings: the default key map, the user's overrides and the merge rules
//! that combine them.
//!
//! Overriding an action replaces the character keys it was bound to, so
//! `start = "S"` frees up `s`. Named keys (arrows, `PageUp`, `Esc`, ...) are
//! kept as soft defaults, so `move_down = "n"` adds `n` without unbinding `↓`.
//! Binding a named key explicitly elsewhere takes it away from its default
//! action; `unbind` drops a key entirely.

mod keyspec;
mod loader;

#[cfg(test)]
mod tests;

pub use keyspec::KeyBind;
#[cfg(test)]
pub use loader::parse;
pub use loader::{DEFAULT_CONFIG_TEMPLATE, load};

use crossterm::event::KeyEvent;

/// The part of the interface a binding applies to. Each context resolves
/// independently, so the same key can mean different things in different views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// Bindings that apply everywhere, plus shared navigation.
    Global,
    /// The service list.
    List,
    /// The log view.
    Log,
    /// The unit-file view.
    UnitFile,
}

impl Context {
    pub const VIEWS: [Self; 3] = [Self::List, Self::Log, Self::UnitFile];

    /// The configuration section this context reads.
    pub const fn section(self) -> &'static str {
        match self {
            Self::Global => "keybindings",
            Self::List => "keybindings.list",
            Self::Log => "keybindings.log",
            Self::UnitFile => "keybindings.unit_file",
        }
    }

    /// The name used for this context inside `[keybindings]`.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Global => "",
            Self::List => "list",
            Self::Log => "log",
            Self::UnitFile => "unit_file",
        }
    }

    /// The actions this context accepts, with their default keys. This table is
    /// also the schema: an action name is valid in a context only if it is
    /// listed here.
    pub const fn defaults(self) -> &'static [(Action, &'static [&'static str])] {
        match self {
            Self::Global => GLOBAL_DEFAULTS,
            Self::List => LIST_DEFAULTS,
            Self::Log => LOG_DEFAULTS,
            Self::UnitFile => UNIT_FILE_DEFAULTS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // Global
    Quit,
    Suspend,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    NavLeft,
    NavRight,
    // List
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
    ToggleMask,
    Refresh,
    ListAllUnits,
    CycleStateFilter,
    ViewLog,
    ViewUnitFile,
    FocusFilter,
    ClearFilter,
    Help,
    // Log
    ToggleAutoRefresh,
    // Unit file
    EditUnit,
    // Log and unit file
    Back,
}

impl Action {
    /// The name used as the configuration key.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Quit => "quit",
            Self::Suspend => "suspend",
            Self::MoveUp => "move_up",
            Self::MoveDown => "move_down",
            Self::PageUp => "page_up",
            Self::PageDown => "page_down",
            Self::NavLeft => "nav_left",
            Self::NavRight => "nav_right",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::ToggleMask => "toggle_mask",
            Self::Refresh => "refresh",
            Self::ListAllUnits => "list_all_units",
            Self::CycleStateFilter => "cycle_state_filter",
            Self::ViewLog => "view_log",
            Self::ViewUnitFile => "view_unit_file",
            Self::FocusFilter => "focus_filter",
            Self::ClearFilter => "clear_filter",
            Self::Help => "help",
            Self::ToggleAutoRefresh => "toggle_auto_refresh",
            Self::EditUnit => "edit",
            Self::Back => "back",
        }
    }

    /// Short label used in the shortcut bar.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Quit => "Exit",
            Self::Suspend => "Suspend",
            Self::MoveUp | Self::MoveDown => "Navigate",
            Self::PageUp | Self::PageDown => "Jump",
            Self::NavLeft | Self::NavRight => "Switch tab",
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
            Self::ToggleMask => "Mask/Unmask",
            Self::Refresh => "Refresh",
            Self::ListAllUnits => "List all units",
            Self::CycleStateFilter => "Filter",
            Self::ViewLog => "Log",
            Self::ViewUnitFile => "Unit File",
            Self::FocusFilter => "Filter text",
            Self::ClearFilter => "Clear filter",
            Self::Help => "Help",
            Self::ToggleAutoRefresh => "Auto-refresh",
            Self::EditUnit => "Edit",
            Self::Back => "Go back",
        }
    }

    /// Longer description used in the help popup.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::Suspend => "Suspend",
            Self::MoveUp => "Move up",
            Self::MoveDown => "Move down",
            Self::PageUp => "Jump 10 items up",
            Self::PageDown => "Jump 10 items down",
            Self::NavLeft => "Previous tab",
            Self::NavRight => "Next tab",
            Self::Start => "Start service",
            Self::Stop => "Stop service",
            Self::Restart => "Restart service",
            Self::Enable => "Enable service",
            Self::Disable => "Disable service",
            Self::ToggleMask => "Mask/Unmask selected unit or template",
            Self::Refresh => "Refresh service list",
            Self::ListAllUnits => "Toggle all/services filter",
            Self::CycleStateFilter => "Cycle filter (all→active→inactive→failed)",
            Self::ViewLog => "View service logs",
            Self::ViewUnitFile => "View unit file details",
            Self::FocusFilter => "Start filtering",
            Self::ClearFilter => "Clear the filter",
            Self::Help => "Toggle this help",
            Self::ToggleAutoRefresh => "Toggle log auto-refresh",
            Self::EditUnit => "Edit the unit file",
            Self::Back => "Go back to the list",
        }
    }

    /// Looks up an action by the name used in a configuration section.
    fn from_name(context: Context, name: &str) -> Option<Self> {
        context
            .defaults()
            .iter()
            .find(|(action, _)| action.name() == name)
            .map(|(action, _)| *action)
    }
}

const GLOBAL_DEFAULTS: &[(Action, &[&str])] = &[
    (Action::Quit, &["ctrl+c"]),
    (Action::Suspend, &["ctrl+z"]),
    (Action::MoveUp, &["up", "k"]),
    (Action::MoveDown, &["down", "j"]),
    (Action::PageUp, &["pageup"]),
    (Action::PageDown, &["pagedown"]),
    (Action::NavLeft, &["left", "h"]),
    (Action::NavRight, &["right", "l"]),
];

const LIST_DEFAULTS: &[(Action, &[&str])] = &[
    (Action::Start, &["s"]),
    (Action::Stop, &["x"]),
    (Action::Restart, &["r"]),
    (Action::Enable, &["e"]),
    (Action::Disable, &["d"]),
    (Action::ToggleMask, &["m"]),
    (Action::Refresh, &["u"]),
    (Action::ListAllUnits, &["f"]),
    (Action::CycleStateFilter, &["a"]),
    (Action::ViewLog, &["v"]),
    (Action::ViewUnitFile, &["c"]),
    (Action::FocusFilter, &["i", "/"]),
    (Action::ClearFilter, &["esc"]),
    (Action::Help, &["?"]),
];

const LOG_DEFAULTS: &[(Action, &[&str])] = &[
    (Action::ToggleAutoRefresh, &["a"]),
    (Action::Back, &["q", "esc"]),
];

const UNIT_FILE_DEFAULTS: &[(Action, &[&str])] =
    &[(Action::EditUnit, &["e"]), (Action::Back, &["q", "esc"])];

/// The keys bound to one action, in the order they should be displayed.
type Binding = (KeyBind, Action);

/// A resolved key map: defaults merged with the user's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBindings {
    global: Vec<Binding>,
    list: Vec<Binding>,
    log: Vec<Binding>,
    unit_file: Vec<Binding>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        // The default table is static and covered by tests, so it cannot
        // conflict with itself.
        Self::resolve(&UserBindings::default()).expect("built-in keybindings are valid")
    }
}

impl KeyBindings {
    fn bindings(&self, context: Context) -> &[Binding] {
        match context {
            Context::Global => &self.global,
            Context::List => &self.list,
            Context::Log => &self.log,
            Context::UnitFile => &self.unit_file,
        }
    }

    /// The action a key event triggers in `context`, if any.
    pub fn action(&self, context: Context, key: KeyEvent) -> Option<Action> {
        let pressed = KeyBind::from_event(key);
        self.bindings(context)
            .iter()
            .find(|(bind, _)| *bind == pressed)
            .map(|(_, action)| *action)
    }

    /// Every key bound to `action` in `context`, in display order.
    pub fn keys(&self, context: Context, action: Action) -> Vec<KeyBind> {
        self.bindings(context)
            .iter()
            .filter(|(_, bound)| *bound == action)
            .map(|(bind, _)| *bind)
            .collect()
    }

    /// The first key bound to `action`, used where space is tight.
    pub fn primary(&self, context: Context, action: Action) -> Option<KeyBind> {
        self.bindings(context)
            .iter()
            .find(|(_, bound)| *bound == action)
            .map(|(bind, _)| *bind)
    }

    /// All keys bound to `action` as `↑/k`, for help text.
    pub fn label(&self, context: Context, action: Action) -> String {
        self.label_with(context, action, "/")
    }

    /// Like [`Self::label`], with a separator that suits running text.
    pub fn label_with(&self, context: Context, action: Action, separator: &str) -> String {
        let keys = self.keys(context, action);
        if keys.is_empty() {
            return "unbound".to_string();
        }
        keys.iter()
            .map(KeyBind::to_string)
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// The first key bound to `action` as display text, falling back to the
    /// full label when nothing is bound.
    pub fn primary_label(&self, context: Context, action: Action) -> String {
        self.primary(context, action)
            .map_or_else(|| "unbound".to_string(), |key| key.to_string())
    }

    /// Merges the user's bindings into the defaults, one context at a time.
    fn resolve(user: &UserBindings) -> Result<Self, String> {
        let global = merge(Context::Global, user)?;
        let list = merge(Context::List, user)?;
        let log = merge(Context::Log, user)?;
        let unit_file = merge(Context::UnitFile, user)?;

        Ok(Self {
            global,
            list,
            log,
            unit_file,
        })
    }
}

/// Bindings read from the configuration file, before being merged with the
/// defaults.
#[derive(Default, Debug)]
pub struct UserBindings {
    /// Explicit bindings per context, in the order they appear in the file.
    bound: Vec<(Context, Action, Vec<KeyBind>)>,
    /// Keys the user wants dropped entirely.
    unbound: Vec<(Context, KeyBind)>,
}

impl UserBindings {
    fn bind(&mut self, context: Context, action: Action, keys: Vec<KeyBind>) {
        self.bound.push((context, action, keys));
    }

    fn unbind(&mut self, context: Context, key: KeyBind) {
        self.unbound.push((context, key));
    }

    /// The contexts whose entries take part in resolving `context`: a view
    /// context also inherits everything from `Global`.
    fn scopes(context: Context) -> &'static [Context] {
        match context {
            Context::Global => &[Context::Global],
            Context::List => &[Context::Global, Context::List],
            Context::Log => &[Context::Global, Context::Log],
            Context::UnitFile => &[Context::Global, Context::UnitFile],
        }
    }

    fn is_unbound(&self, context: Context, key: KeyBind) -> bool {
        let scopes = Self::scopes(context);
        self.unbound
            .iter()
            .any(|(scope, unbound)| scopes.contains(scope) && *unbound == key)
    }

    fn explicit(&self, context: Context) -> impl Iterator<Item = (Action, &[KeyBind])> {
        let scopes = Self::scopes(context);
        self.bound
            .iter()
            .filter(move |(scope, _, _)| scopes.contains(scope))
            .map(|(_, action, keys)| (*action, keys.as_slice()))
    }

    fn is_explicit(&self, context: Context, action: Action) -> bool {
        self.explicit(context).any(|(bound, _)| bound == action)
    }
}

/// Applies the merge rules for a single context. Explicit bindings are laid
/// down first so that they win over defaults, then each default key is added
/// back unless the user took it or replaced it.
fn merge(context: Context, user: &UserBindings) -> Result<Vec<Binding>, String> {
    let mut resolved: Vec<Binding> = Vec::new();

    for (action, keys) in user.explicit(context) {
        for key in keys {
            if user.is_unbound(context, *key) {
                return Err(format!(
                    "[{}]: '{key}' is listed in `unbind` but also bound to `{}`",
                    context.section(),
                    action.name()
                ));
            }
            if let Some((_, previous)) = resolved.iter().find(|(bound, _)| bound == key) {
                return Err(format!(
                    "[{}]: '{key}' is bound to both `{}` and `{}`",
                    context.section(),
                    previous.name(),
                    action.name()
                ));
            }
            resolved.push((*key, action));
        }
    }

    for scope in UserBindings::scopes(context) {
        for (action, specs) in scope.defaults() {
            let replaced = user.is_explicit(context, *action);

            for spec in *specs {
                let key = KeyBind::parse(spec)
                    .map_err(|error| format!("invalid built-in binding '{spec}': {error}"))?;

                // An overridden action keeps only its named default keys; its
                // character keys are freed up for other actions.
                if replaced && key.is_character() {
                    continue;
                }
                if user.is_unbound(context, key) {
                    continue;
                }

                match resolved.iter().find(|(bound, _)| *bound == key) {
                    None => resolved.push((key, *action)),
                    // The user moved a named key, which silently takes it away
                    // from its default action.
                    Some(_) if !key.is_character() => {}
                    Some((_, owner)) => {
                        return Err(format!(
                            "[{}]: '{key}' is bound to `{}`, but it is also the default key for \
                             `{}`. Bind `{}` to another key (or add it to `unbind`).",
                            context.section(),
                            owner.name(),
                            action.name(),
                            action.name()
                        ));
                    }
                }
            }
        }
    }

    Ok(resolved)
}
