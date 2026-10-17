use super::*;

impl ModuleSettingsPage {
    pub(super) fn render_field(
        &self,
        field: &SettingDescriptor,
        value: &Value,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Div {
        if let SettingKind::BooleanMap { .. } = field.kind {
            return self.render_bool_map(field, value, cx);
        }
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .min_h(px(22.0))
            .py(px(1.0));
        match &field.kind {
            SettingKind::Boolean => {
                let key = field.key.clone();
                let checked = value.as_bool().unwrap_or(false);
                let page = cx.entity().downgrade();
                row = row
                    .child(
                        SemanticCheckbox::new(format!("setting-{key}"))
                            .checked(checked)
                            .accessibility_label(field.label.clone())
                            .size(px(13.0))
                            .on_change(move |next, _, _, cx| {
                                let _ = page.update(cx, |page, cx| {
                                    page.set_value(
                                        &key,
                                        json!(matches!(next, CheckboxState::Checked)),
                                        cx,
                                    );
                                });
                            })
                            .child(checkbox_indicator(checked)),
                    )
                    .child(label(&field.label, TEXT, 12.0));
            }
            SettingKind::Map => {
                row = row.child(field_label(&field.label));
                if let Some(select) = self.map_selects.get(&field.key) {
                    row = row.child(
                        Select::new(select)
                            .xsmall()
                            .w(px(260.0))
                            .h(px(23.0))
                            .px(px(6.0))
                            .bg(rgb(PANEL_DARK))
                            .border_1()
                            .border_color(rgb(LINE))
                            .rounded(px(1.0))
                            .text_size(px(11.5))
                            .text_color(rgb(TEXT)),
                    );
                }
            }
            SettingKind::Choice { options } => {
                row = row.child(field_label(&field.label));
                let mut choices = div().flex().flex_wrap().items_center().gap(px(3.0));
                let visible_options: Vec<_> = if field.key == "pet_gear" {
                    self.pet_gears.as_ref().map_or_else(
                        || options.iter().collect(),
                        |gears| {
                            options
                                .iter()
                                .filter(|option| {
                                    option.value.is_null()
                                        || option
                                            .value
                                            .as_i64()
                                            .and_then(|gear| i32::try_from(gear).ok())
                                            .is_some_and(|gear| gears.contains(&gear))
                                })
                                .collect()
                        },
                    )
                } else {
                    options.iter().collect()
                };
                for (index, option) in visible_options.iter().enumerate() {
                    let selected = option.value == *value;
                    let key = field.key.clone();
                    let choice = option.value.clone();
                    choices = choices.child(
                        div()
                            .id(format!("choice-{key}-{index}"))
                            .h(px(21.0))
                            .px(px(6.0))
                            .flex()
                            .items_center()
                            .bg(rgb(if selected { BLUE } else { PANEL_DARK }))
                            .border_1()
                            .border_color(rgb(if selected { 0x6d9bd2 } else { LINE }))
                            .rounded(px(1.0))
                            .cursor_pointer()
                            .text_size(px(11.5))
                            .text_color(rgb(if selected { 0xffffff } else { TEXT }))
                            .on_click(cx.listener(move |page, _, _, cx| {
                                page.set_value(&key, choice.clone(), cx);
                            }))
                            .child(option.label.clone()),
                    );
                }
                row = row.child(choices);
                if field.key == "pet_gear" && self.pet_gears.is_none() {
                    row = row.child(label(
                        "Pet gear inventory unavailable; selection waits for an observation.",
                        FAINT,
                        11.0,
                    ));
                }
            }
            _ => {
                row = row.child(field_label(&field.label));
                if let Some(input) = self.inputs.get(&field.key) {
                    let width = match field.kind {
                        SettingKind::Integer { .. } | SettingKind::Number { .. } => 72.0,
                        SettingKind::Json => 420.0,
                        _ => 200.0,
                    };
                    row = row.child(Input::new(input).xsmall().w(px(width)));
                }
            }
        }
        if let Some(error) = self.errors.get(&field.key) {
            row = row.child(label(error, 0xf0b9b9, 12.0));
        } else if !field.description.is_empty() {
            row = row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(label(&field.description, FAINT, 12.0)),
            );
        }
        row
    }

    pub(super) fn render_bool_map(
        &self,
        field: &SettingDescriptor,
        value: &Value,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Div {
        let mut block = div().flex().flex_col().py(px(2.0));
        if !field.label.is_empty() {
            block = block.child(div().h(px(18.0)).flex().items_center().child(label(
                &field.label,
                MUTED,
                12.0,
            )));
        }
        let Some(map) = value.as_object() else {
            return block.child(label("Invalid list", 0xf0b9b9, 12.0));
        };
        let query = self
            .map_search_inputs
            .get(&field.key)
            .map(|input| input.read(cx).value().to_string().to_lowercase())
            .unwrap_or_default();
        if map.is_empty() {
            block = block.child(label(
                "None yet — names show up here as they are seen.",
                FAINT,
                12.0,
            ));
        }
        if let Some(search) = self.map_search_inputs.get(&field.key) {
            block = block.child(
                div()
                    .pb(px(4.0))
                    .child(Input::new(search).xsmall().w_full()),
            );
        }
        block = block.child(
            div()
                .h(px(22.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(6.0))
                .bg(rgb(PANEL_DARK))
                .border_b_1()
                .border_color(rgb(LINE))
                .child(div().flex_1().child(label("Name", MUTED, 12.0)))
                .child(div().w(px(76.0)).child(label("Enabled", MUTED, 12.0)))
                .child(div().w(px(116.0)).child(label("Extra flags", MUTED, 12.0))),
        );
        for (name, enabled) in map {
            if !query.is_empty() && !name.to_lowercase().contains(&query) {
                continue;
            }
            let key = field.key.clone();
            let entry = name.clone();
            let checked = enabled.as_bool().unwrap_or(false);
            let page = cx.entity().downgrade();
            block = block.child(
                div()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(6.0))
                    .border_b_1()
                    .border_color(rgb(LINE_SOFT))
                    .child(div().flex_1().child(label(name, TEXT, 12.0)))
                    .child(
                        div().w(px(76.0)).child(
                            SemanticCheckbox::new(format!("entry-{key}-{entry}"))
                                .checked(checked)
                                .accessibility_label(entry.clone())
                                .size(px(13.0))
                                .on_change(move |next, _, _, cx| {
                                    let _ = page.update(cx, |page, cx| {
                                        let mut value = setting_value(&page.values, &key)
                                            .cloned()
                                            .unwrap_or_else(|| json!({}));
                                        if let Some(map) = value.as_object_mut() {
                                            map.insert(
                                                entry.clone(),
                                                json!(matches!(next, CheckboxState::Checked)),
                                            );
                                        }
                                        page.set_value(&key, value, cx);
                                    });
                                })
                                .child(legacy_checkbox_indicator(checked)),
                        ),
                    )
                    .child(div().w(px(116.0)).child(label("—", FAINT, 12.0))),
            );
        }
        if !field.description.is_empty() {
            block = block.child(label(&field.description, FAINT, 12.0));
        }
        if let Some(error) = self.errors.get(&field.key) {
            block = block.child(label(error, 0xf0b9b9, 12.0));
        }
        block
    }
}

fn field_label(text: &str) -> gpui_kit::Div {
    div()
        .w(px(210.0))
        .flex_shrink_0()
        .text_size(px(12.0))
        .text_color(rgb(TEXT))
        .child(text.to_string())
}

/// the reference implementation's settings use a compact, nearly square native-style checkbox.
/// The semantic control owns keyboard and accessibility behavior; this child
/// owns only its flat visual indicator.
fn checkbox_indicator(checked: bool) -> impl IntoElement {
    CheckboxIndicator::new()
        .checked(checked)
        .w(px(13.0))
        .h(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(if checked { BLUE } else { PANEL_DARK }))
        .border_1()
        .border_color(rgb(if checked { 0x6d9bd2 } else { 0x85898b }))
        .rounded(px(1.0))
        .text_size(px(10.0))
        .text_color(rgb(TEXT))
        .child(if checked { "✓" } else { "" })
}
