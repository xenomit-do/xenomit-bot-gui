//! Hotkey values for the key-capture widget (DarkBot's `CharacterEditor`).
//!
//! A key is stored as a string: one printable character (`"2"`, `"x"`, `"-"`)
//! or the name of a key that has no character (`"F1"`, `"Enter"`, `"Up"`).
//! Names are case-insensitive on input and canonical when stored.

/// Keys without a printable character, in display order.
const NAMED: &[&str] = &[
    "Enter", "Tab", "Insert", "Home", "End", "PageUp", "PageDown", "Up", "Down", "Left", "Right",
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
];

/// What a key press means to the widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Press {
    /// Bind this key.
    Key(String),
    /// Backspace/Delete: remove the binding.
    Clear,
    /// Escape: leave capture mode without changing anything.
    Cancel,
    /// Not usable as a hotkey (bare modifier, unknown key).
    Ignore,
}

/// Canonical stored form of `text`, or `None` if it is not a valid key.
pub(crate) fn normalize(text: &str) -> Option<String> {
    let mut chars = text.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return (!only.is_control()).then(|| only.to_string());
    }
    NAMED
        .iter()
        .find(|name| name.eq_ignore_ascii_case(text.trim()))
        .map(|name| (*name).to_owned())
}

/// Interprets a GPUI keystroke. `key` is the base key (`"2"`, `"f1"`,
/// `"enter"`), `key_char` the typed character if there is one.
pub(crate) fn interpret(key: &str, key_char: Option<&str>) -> Press {
    match key {
        "escape" => return Press::Cancel,
        "backspace" | "delete" => return Press::Clear,
        "space" => return Press::Key(" ".into()),
        "shift" | "control" | "alt" | "platform" | "function" | "capslock" | "numlock" => {
            return Press::Ignore;
        }
        _ => {}
    }
    // Base key first: Shift+2 should bind "2", the physical hotkey.
    let candidate = if key.chars().count() == 1 {
        key.to_lowercase()
    } else {
        key.to_owned()
    };
    if let Some(named) = normalize(&candidate)
        && named.chars().count() > 1
    {
        return Press::Key(named);
    }
    match normalize(&candidate) {
        Some(single) => Press::Key(single),
        None => key_char
            .and_then(normalize)
            .map_or(Press::Ignore, Press::Key),
    }
}

/// Text shown in the widget for a stored key.
pub(crate) fn display(key: &str) -> String {
    match key {
        " " => "Space".into(),
        single if single.chars().count() == 1 => single.to_uppercase(),
        named => named.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_characters_and_names() {
        assert_eq!(normalize("2").as_deref(), Some("2"));
        assert_eq!(normalize("x").as_deref(), Some("x"));
        assert_eq!(normalize(" ").as_deref(), Some(" "));
        assert_eq!(normalize("f5").as_deref(), Some("F5"));
        assert_eq!(normalize("PAGEUP").as_deref(), Some("PageUp"));
        assert_eq!(normalize("up").as_deref(), Some("Up"));
        assert_eq!(normalize("\n"), None);
        assert_eq!(normalize("ab"), None);
        assert_eq!(normalize("F25"), None);
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn interprets_keystrokes() {
        assert_eq!(
            interpret("2", Some("@")),
            Press::Key("2".into()),
            "base key wins over Shift"
        );
        assert_eq!(interpret("a", Some("a")), Press::Key("a".into()));
        assert_eq!(interpret("A", None), Press::Key("a".into()));
        assert_eq!(interpret("f1", None), Press::Key("F1".into()));
        assert_eq!(interpret("enter", Some("\n")), Press::Key("Enter".into()));
        assert_eq!(interpret("pageup", None), Press::Key("PageUp".into()));
        assert_eq!(interpret("space", Some(" ")), Press::Key(" ".into()));
        assert_eq!(interpret("escape", None), Press::Cancel);
        assert_eq!(interpret("backspace", None), Press::Clear);
        assert_eq!(interpret("delete", None), Press::Clear);
        assert_eq!(interpret("shift", None), Press::Ignore);
        assert_eq!(interpret("unknownkey", None), Press::Ignore);
        assert_eq!(interpret("unknownkey", Some("q")), Press::Key("q".into()));
    }

    #[test]
    fn displays_readable_names() {
        assert_eq!(display(" "), "Space");
        assert_eq!(display("x"), "X");
        assert_eq!(display("F1"), "F1");
        assert_eq!(display("PageUp"), "PageUp");
    }
}
