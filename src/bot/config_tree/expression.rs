//! Parsers for condition expressions and extra actions used by the settings editor.
use serde_json::{Value, json};
use xenomit_game_api::GameCommand;

/// Parse supported condition expressions into the typed JSON representation.
/// Empty input means an unconditional match.
/// Unsupported names or malformed arguments return a validation error; they
/// are never silently treated as true.
pub(crate) fn parse_condition_expression(source: &str) -> Result<Value, String> {
    let source = source.trim();
    if source.is_empty() || source.eq_ignore_ascii_case("true") {
        return Ok(json!({}));
    }
    let open = source
        .find('(')
        .ok_or_else(|| "Expected condition as name(arguments)".to_owned())?;
    if !source.ends_with(')') {
        return Err("Condition is missing its closing ')'".into());
    }
    let name = source[..open].trim().to_ascii_lowercase().replace('_', "-");
    let args = split_condition_args(&source[open + 1..source.len() - 1])?;
    let one = || {
        if args.len() == 1 {
            Ok(args[0].as_str())
        } else {
            Err(format!("{name} expects one argument"))
        }
    };
    let two = || {
        if args.len() == 2 {
            Ok((args[0].as_str(), args[1].as_str()))
        } else {
            Err(format!("{name} expects two arguments"))
        }
    };
    match name.as_str() {
        "all" | "any" | "none" => {
            let parsed = args
                .iter()
                .map(|arg| parse_condition_expression(arg))
                .collect::<Result<Vec<_>, _>>()?;
            let key = name.replace('-', "_");
            Ok(json!({ (key): parsed }))
        }
        "not" => Ok(json!({"not": parse_condition_expression(one()?)?})),
        "hp-below" | "shield-below" => {
            let value = parse_condition_number(one()?)?;
            if !(0.0..=1.0).contains(&value) {
                return Err(format!("{name} expects a fraction between 0 and 1"));
            }
            let key = name.replace('-', "_");
            Ok(json!({ (key): value }))
        }
        "attacking" => {
            let value = one()?
                .parse::<bool>()
                .map_err(|_| "attacking expects true or false")?;
            Ok(json!({"attacking": value}))
        }
        "map" => {
            let raw = one()?.trim().trim_matches(['"', '\'']);
            let id = raw
                .parse::<i64>()
                .ok()
                .or_else(|| {
                    xenomit_game_api::star_map::MAPS
                        .iter()
                        .find(|(_, name, _)| name.eq_ignore_ascii_case(raw))
                        .map(|(id, _, _)| i64::from(*id))
                })
                .ok_or_else(|| "map expects a known map id or name".to_owned())?;
            Ok(json!({"map": id}))
        }
        "map-time" | "elapsed" => {
            let seconds = parse_condition_number(one()?)?;
            if seconds < 0.0 || seconds > u64::MAX as f64 / 1000.0 {
                return Err(format!("{name} expects a non-negative duration in seconds"));
            }
            let key = if name == "map-time" {
                "map_time_ms"
            } else {
                "elapsed_ms"
            };
            Ok(json!({key: (seconds * 1000.0).round() as u64}))
        }
        "deaths" => Ok(json!({"deaths": parse_condition_u64(one()?)?})),
        "target-name" | "key-empty" => {
            let value = parse_condition_string(one()?);
            let key = if name == "target-name" {
                "target_name"
            } else {
                "key_empty"
            };
            Ok(json!({key: value}))
        }
        "field" => {
            let (key, value) = two()?;
            let key = parse_condition_string(key);
            let value = parse_condition_scalar(value)?;
            Ok(json!({"field": {(key): value}}))
        }
        "npc-kills" | "resources" => {
            let (key, count) = two()?;
            let key = parse_condition_string(key);
            let count = parse_condition_u64(count)?;
            let field = if name == "npc-kills" {
                "npc_kills"
            } else {
                "resources"
            };
            Ok(json!({(field): {(key): count}}))
        }
        "time" => {
            let (hour, minute) = two()?;
            let hour = parse_condition_u64(hour)?;
            let minute = parse_condition_u64(minute)?;
            if hour > 23 || minute > 59 {
                return Err("time expects an hour from 0-23 and minute from 0-59".into());
            }
            Ok(json!({"time": {"hour": hour, "minute": minute}}))
        }
        _ => Err(format!("Unsupported condition '{name}'")),
    }
}

/// Parse the supported Extra Actions syntax into an existing typed command or
/// host action. The returned shape can be stored directly in a custom-event
/// row and is validated again by the behavior registry at runtime.
pub(crate) fn parse_action_expression(source: &str) -> Result<Value, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("Choose a supported action".into());
    }
    let (name, args) = if let Some(open) = source.find('(') {
        if !source.ends_with(')') {
            return Err("Action is missing its closing ')'".into());
        }
        (
            source[..open].trim().to_ascii_lowercase().replace('_', "-"),
            split_condition_args(&source[open + 1..source.len() - 1])?,
        )
    } else {
        (source.to_ascii_lowercase().replace('_', "-"), Vec::new())
    };
    let arg = |index: usize| {
        args.get(index)
            .map(String::as_str)
            .ok_or_else(|| format!("{name} is missing argument {}", index + 1))
    };
    let exact = |count: usize| {
        if args.len() == count {
            Ok(())
        } else {
            Err(format!("{name} expects {count} argument(s)"))
        }
    };
    let parse_int = |index: usize| {
        parse_condition_u64(arg(index)?)
            .and_then(|n| i64::try_from(n).map_err(|_| "Integer is too large".into()))
    };
    let command = match name.as_str() {
        "use-item" => {
            if !(1..=2).contains(&args.len()) {
                return Err("use-item expects item-id and optional activate flag".into());
            }
            let item_id = parse_condition_string(arg(0)?);
            let activate = if let Some(value) = args.get(1) {
                value
                    .parse::<bool>()
                    .map_err(|_| "use-item activate must be true or false")?
            } else {
                true
            };
            json!({"command":"use_item","item_id":item_id,"activate":activate})
        }
        "refine-ore" | "sell-ore" => {
            exact(2)?;
            let ore_type = parse_int(0)?;
            let amount = parse_condition_u64(arg(1)?)?;
            if name == "refine-ore" {
                json!({"command":"refine_ore","ore_type":ore_type,"amount":amount})
            } else {
                json!({"command":"sell_ore","ore_type":ore_type,"amount":amount})
            }
        }
        "jump-portal" => {
            exact(1)?;
            json!({"command":"jump_portal","portal_id":parse_int(0)?})
        }
        "set-pet" => {
            exact(1)?;
            let enabled = arg(0)?
                .parse::<bool>()
                .map_err(|_| "set-pet expects true or false")?;
            json!({"command":"set_pet","enabled":enabled})
        }
        "select-configuration" => {
            exact(1)?;
            let configuration = parse_int(0)?;
            if !(1..=2).contains(&configuration) {
                return Err("configuration must be 1 or 2".into());
            }
            json!({"command":"select_configuration","configuration":configuration})
        }
        "select-pet-gear" => {
            if !(1..=2).contains(&args.len()) {
                return Err("select-pet-gear expects gear-type and optional target".into());
            }
            json!({"command":"select_pet_gear","gear_type":parse_int(0)?,"target":if args.len()==2 {parse_int(1)?} else {0}})
        }
        "accept-group-invite" => {
            exact(1)?;
            json!({"command":"accept_group_invite","inviter_id":parse_int(0)?})
        }
        "invite-group-member" => {
            exact(1)?;
            json!({"command":"invite_group_member","player_id":parse_int(0)?})
        }
        "kick-group-member" => {
            exact(1)?;
            json!({"command":"kick_group_member","player_id":parse_int(0)?})
        }
        "leave-group" => {
            exact(0)?;
            json!({"command":"leave_group"})
        }
        "set-group-invites" => {
            exact(1)?;
            let open = arg(0)?
                .parse::<bool>()
                .map_err(|_| "set-group-invites expects true or false")?;
            json!({"command":"set_group_invites","open":open})
        }
        "refresh" => {
            exact(0)?;
            return Ok(json!({"host_action":"refresh"}));
        }
        "disconnect" => {
            exact(0)?;
            return Ok(json!({"host_action":"disconnect"}));
        }
        "close" => {
            exact(0)?;
            return Ok(json!({"host_action":"close"}));
        }
        "profile" => {
            exact(1)?;
            return Ok(json!({"host_action":"profile","name":parse_condition_string(arg(0)?)}));
        }
        "accept-urgent-quest" => {
            exact(0)?;
            return Ok(json!({"host_action":"accept_urgent_quest"}));
        }
        "claim-calendar" => {
            exact(0)?;
            return Ok(json!({"host_action":"claim_calendar"}));
        }
        _ => return Err(format!("Unsupported Extra Action '{name}'")),
    };
    let _: GameCommand = serde_json::from_value(command.clone())
        .map_err(|e| format!("Invalid typed action: {e}"))?;
    Ok(json!({"command":command}))
}

fn split_condition_args(source: &str) -> Result<Vec<String>, String> {
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (mut args, mut start, mut depth, mut quote, mut escaped) =
        (Vec::new(), 0usize, 0i32, None, false);
    for (index, ch) in source.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(source[start..index].trim().to_owned());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        if depth < 0 {
            return Err("Unexpected ')' in condition".into());
        }
    }
    if quote.is_some() || depth != 0 {
        return Err("Unclosed string or nested condition".into());
    }
    args.push(source[start..].trim().to_owned());
    if args.iter().any(String::is_empty) {
        return Err("Condition arguments cannot be empty".into());
    }
    Ok(args)
}

fn parse_condition_string(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let first = value.as_bytes()[0] as char;
        if (first == '"' || first == '\'') && value.ends_with(first) {
            if first == '"' {
                return serde_json::from_str(value)
                    .unwrap_or_else(|_| value[1..value.len() - 1].into());
            }
            return value[1..value.len() - 1]
                .replace("\\'", "'")
                .replace("\\\\", "\\");
        }
    }
    value.to_owned()
}

fn parse_condition_scalar(value: &str) -> Result<Value, String> {
    let value = value.trim();
    if value.starts_with('"') {
        return serde_json::from_str(value).map_err(|_| "Invalid quoted string".into());
    }
    if value.starts_with('\'') {
        return Ok(json!(parse_condition_string(value)));
    }
    serde_json::from_str(value)
        .map_err(|_| "field value must be a quoted string, number, or boolean".into())
}
fn parse_condition_number(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| "Expected a finite number".into())
}
fn parse_condition_u64(value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| "Expected a non-negative whole number".into())
}
