use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fmt;

/// Modifiers that take part in binding comparison. Everything else reported by
/// the terminal (`KEYPAD`, `HYPER`, ...) is dropped so a binding keeps matching
/// on terminals that report extra state.
const MEANINGFUL_MODIFIERS: KeyModifiers = KeyModifiers::CONTROL
    .union(KeyModifiers::ALT)
    .union(KeyModifiers::SUPER)
    .union(KeyModifiers::SHIFT);

/// A single key combination an action can be bound to, normalized so that a
/// parsed configuration entry and a key event from the terminal compare equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBind {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBind {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let (code, modifiers) = normalize(code, modifiers);
        Self { code, modifiers }
    }

    pub fn from_event(event: KeyEvent) -> Self {
        Self::new(event.code, event.modifiers)
    }

    /// Character keys replace the defaults of the action they are bound to;
    /// named keys (arrows, `PageUp`, `Esc`, ...) stay bound as soft defaults.
    pub const fn is_character(self) -> bool {
        matches!(self.code, KeyCode::Char(_))
    }

    /// Whether this key produces text. Such a key has to reach a focused
    /// input instead of triggering a global action.
    pub fn is_text(self) -> bool {
        self.is_character() && self.modifiers.is_empty()
    }

    /// Parses a configuration key specification such as `s`, `ctrl+c`,
    /// `alt+f`, `down` or `f5`.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Err("empty key specification".to_string());
        }

        let tokens = split_tokens(trimmed);
        let (key_token, modifier_tokens) = tokens
            .split_last()
            .ok_or_else(|| format!("'{spec}' is not a valid key"))?;

        // A single-token specification is already quoted in the caller's
        // message, so only a chord needs to name the whole thing again.
        let context = if tokens.len() > 1 {
            format!(" in '{trimmed}'")
        } else {
            String::new()
        };

        let mut modifiers = KeyModifiers::NONE;
        for token in modifier_tokens {
            modifiers |= match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "meta" | "option" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "super" | "cmd" | "win" => KeyModifiers::SUPER,
                other => {
                    return Err(format!(
                        "unknown modifier '{other}'{context} (expected ctrl, alt, shift or super)"
                    ));
                }
            };
        }

        Ok(Self::new(parse_code(key_token, &context)?, modifiers))
    }
}

impl fmt::Display for KeyBind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (modifier, name) in [
            (KeyModifiers::CONTROL, "Ctrl"),
            (KeyModifiers::ALT, "Alt"),
            (KeyModifiers::SUPER, "Super"),
            (KeyModifiers::SHIFT, "Shift"),
        ] {
            if self.modifiers.contains(modifier) {
                write!(f, "{name}+")?;
            }
        }

        match self.code {
            KeyCode::Char(' ') => f.write_str("Space"),
            KeyCode::Char(character) => write!(f, "{character}"),
            KeyCode::Up => f.write_str("↑"),
            KeyCode::Down => f.write_str("↓"),
            KeyCode::Left => f.write_str("←"),
            KeyCode::Right => f.write_str("→"),
            KeyCode::PageUp => f.write_str("PageUp"),
            KeyCode::PageDown => f.write_str("PageDown"),
            KeyCode::Home => f.write_str("Home"),
            KeyCode::End => f.write_str("End"),
            KeyCode::Insert => f.write_str("Insert"),
            KeyCode::Delete => f.write_str("Delete"),
            KeyCode::Backspace => f.write_str("Backspace"),
            KeyCode::Enter => f.write_str("Enter"),
            KeyCode::Tab => f.write_str("Tab"),
            KeyCode::BackTab => f.write_str("BackTab"),
            KeyCode::Esc => f.write_str("Esc"),
            KeyCode::F(number) => write!(f, "F{number}"),
            other => write!(f, "{other:?}"),
        }
    }
}

fn normalize(code: KeyCode, modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    let mut modifiers = modifiers & MEANINGFUL_MODIFIERS;
    let mut code = code;

    if let KeyCode::Char(character) = code {
        // The character already encodes Shift ('S' is not 's'), so keeping the
        // modifier would make `S` unmatchable.
        modifiers.remove(KeyModifiers::SHIFT);
        // Terminals disagree on whether a Ctrl/Alt chord reports the shifted
        // character, so chords are matched case-insensitively.
        if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            code = KeyCode::Char(character.to_ascii_lowercase());
        }
    }

    (code, modifiers)
}

/// Splits `ctrl+alt+x` into its tokens. A trailing empty token comes from a
/// literal plus sign (`ctrl++`), so it is folded back into one.
fn split_tokens(spec: &str) -> Vec<String> {
    if spec == "+" {
        return vec!["+".to_string()];
    }

    let mut tokens: Vec<String> = spec.split('+').map(str::to_string).collect();
    if tokens.len() >= 2 && tokens.last().is_some_and(String::is_empty) {
        tokens.pop();
        if let Some(last) = tokens.last_mut()
            && last.is_empty()
        {
            *last = "+".to_string();
        }
    }
    tokens
}

fn parse_code(token: &str, context: &str) -> Result<KeyCode, String> {
    let mut characters = token.chars();
    if let (Some(character), None) = (characters.next(), characters.next()) {
        return Ok(KeyCode::Char(character));
    }

    let name = token.to_ascii_lowercase();

    if let Some(number) = name.strip_prefix('f')
        && let Ok(number) = number.parse::<u8>()
        && (1..=12).contains(&number)
    {
        return Ok(KeyCode::F(number));
    }

    Ok(match name.as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pageup" | "page_up" | "pgup" => KeyCode::PageUp,
        "pagedown" | "page_down" | "pgdn" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "insert" | "ins" => KeyCode::Insert,
        "delete" | "del" => KeyCode::Delete,
        "backspace" | "bs" => KeyCode::Backspace,
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" | "shift_tab" => KeyCode::BackTab,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        _ => return Err(format!("unknown key '{token}'{context}")),
    })
}
