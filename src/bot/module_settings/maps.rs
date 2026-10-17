use super::*;

#[derive(Clone)]
pub(crate) struct MapChoice {
    pub(crate) label: String,
    pub(crate) value: Value,
}
impl SelectItem for MapChoice {
    type Value = Value;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &Value {
        &self.value
    }
}
pub(crate) fn map_choices(
    maps: &[xenomit_game_api::MapDescriptor],
    selected: &Value,
) -> Vec<MapChoice> {
    let selected = resolved_map_value(maps, selected);
    let mut choices = vec![MapChoice {
        label: "Current map".into(),
        value: Value::Null,
    }];
    choices.extend(maps.iter().map(|map| MapChoice {
        label: format!("{} (ID {}, level {})", map.name, map.id, map.min_level),
        value: json!(map.id),
    }));
    choices.extend(
        xenomit_game_api::star_map::MAPS
            .iter()
            .filter(|(id, _, _)| !maps.iter().any(|map| map.id == *id))
            .map(|(id, name, _)| MapChoice {
                label: format!("{} (ID {})", name, id),
                value: json!(id),
            }),
    );
    // Keep a legacy string selection until the user chooses an ID. This also
    // preserves the display value while the client/catalog is still loading.
    if !choices.iter().any(|choice| choice.value == selected) {
        choices.push(MapChoice {
            label: selected
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| selected.to_string()),
            value: selected,
        });
    }
    choices
}
pub(crate) fn resolved_map_value(
    maps: &[xenomit_game_api::MapDescriptor],
    selected: &Value,
) -> Value {
    let Some(name) = selected.as_str() else {
        return selected.clone();
    };
    maps.iter()
        .find(|map| map.name.eq_ignore_ascii_case(name.trim()))
        .map(|map| json!(map.id))
        .or_else(|| {
            xenomit_game_api::star_map::MAPS
                .iter()
                .find(|(_, map_name, _)| map_name.eq_ignore_ascii_case(name.trim()))
                .map(|(id, _, _)| json!(id))
        })
        .or_else(|| name.trim().parse::<i32>().ok().map(|id| json!(id)))
        .unwrap_or_else(|| selected.clone())
}
