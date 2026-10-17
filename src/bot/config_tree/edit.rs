//! Text <-> value conversion for the editors backed by a text input.
use super::schema::Kind;
use serde_json::{Value, json};

/// Trims trailing zeros: `5.0` -> `5`, `0.25` -> `0.25`.
pub(crate) fn format_number(number: f64) -> String {
    if number.fract().abs() < 1e-9 {
        format!("{}", number.round() as i64)
    } else {
        let text = format!("{number:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// Text shown in an input for `value` of `kind`.
pub(crate) fn format_value(kind: &Kind, value: &Value) -> String {
    match kind {
        Kind::Int { .. } | Kind::Float { .. } => {
            value.as_f64().map(format_number).unwrap_or_default()
        }
        Kind::Percent { .. } => value
            .as_f64()
            .map(|fraction| format_number(fraction * 100.0))
            .unwrap_or_default(),
        Kind::Char | Kind::Text | Kind::Condition | Kind::Color { .. } => {
            value.as_str().unwrap_or_default().to_owned()
        }
        Kind::Json => {
            if value.is_null() {
                String::new()
            } else {
                value.to_string()
            }
        }
        _ => String::new(),
    }
}

/// Parses input text into a normalized value, or explains why it is invalid.
pub(crate) fn parse_text(kind: &Kind, text: &str) -> Result<Value, String> {
    let value = match kind {
        Kind::Int { .. } => json!(
            text.trim()
                .parse::<i64>()
                .map_err(|_| "Enter a whole number".to_owned())?
        ),
        Kind::Float { .. } => json!(parse_f64(text)?),
        Kind::Percent { .. } => json!(parse_f64(text)? / 100.0),
        Kind::Char => {
            if text.is_empty() {
                Value::Null
            } else {
                json!(super::keys::normalize(text).ok_or("Not a valid key")?)
            }
        }
        Kind::Text => json!(text),
        Kind::Condition => {
            xenomit_behavior_engine::parse_rule_condition(text)?;
            json!(text)
        }
        Kind::Color { .. } => json!(text),
        Kind::Json => {
            if text.trim().is_empty() {
                Value::Null
            } else {
                serde_json::from_str(text).map_err(|error| format!("Invalid JSON: {error}"))?
            }
        }
        _ => return Err("This option is not edited as text".into()),
    };
    kind.normalize(&value)
        .ok_or_else(|| "Invalid value".to_owned())
}

fn parse_f64(text: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| "Enter a number".to_owned())
}

/// Step applied by the +/- buttons, in the value's own units.
pub(crate) fn step_of(kind: &Kind) -> Option<f64> {
    match kind {
        Kind::Int { step, .. } => Some(*step as f64),
        Kind::Float { step, .. } | Kind::Percent { step, .. } => Some(*step),
        _ => None,
    }
}
