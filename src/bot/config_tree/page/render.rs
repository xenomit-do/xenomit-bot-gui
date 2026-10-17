use super::*;

impl Render for ConfigTreePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(name) = self.pending_profile.take() {
            self.switch_profile(&name, window, cx);
        }
        self.resync_stale(window, cx);
        let toolbar = self.render_toolbar(window, cx);
        let query = self.input_text(&Role::Search, cx);
        let filter = Filter::new(self.level, &query);
        let tabs = self.render_tabs(&filter, cx);
        let content = self.render_content(&filter, window, cx);
        let footer = self.render_footer();

        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .child(toolbar)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .child(tabs)
                    .child(content),
            )
            .child(footer)
    }
}

fn indicator(checked: bool) -> impl IntoElement {
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

fn role_path(role: &Role) -> Option<&str> {
    match role {
        Role::Value(path) | Role::Range { path, .. } | Role::Cell { path, .. } => Some(path),
        Role::NewRow(_) | Role::Search | Role::ProfileName => None,
    }
}

fn field_label(text: &str, width: f32) -> gpui_kit::Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_size(px(12.0))
        .text_color(rgb(TEXT))
        .child(text.to_owned())
}

fn parse_hex(text: &str) -> u32 {
    u32::from_str_radix(
        text.trim_start_matches('#').get(..6).unwrap_or("000000"),
        16,
    )
    .unwrap_or(0)
}

impl ConfigTreePage {
    /// Edits made through buttons/checkboxes/resets leave the retained text
    /// inputs stale; typing never marks them, so the caret is not disturbed.
    fn mark_stale(&mut self, path: &str) {
        self.stale.insert(path.to_owned());
    }

    fn set_external(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        self.mark_stale(path);
        self.set(path, value, cx);
    }

    fn resync_stale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stale.is_empty() {
            return;
        }
        let stale = std::mem::take(&mut self.stale);
        let targets: Vec<(Entity<InputState>, Role)> = self
            .inputs
            .values()
            .filter(|(_, role)| role_path(role).is_some_and(|path| stale.contains(path)))
            .cloned()
            .collect();
        for (input, role) in targets {
            let text = self.text_for(&role);
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    fn reset_path(&mut self, path: &str, cx: &mut Context<Self>) {
        self.mark_stale(path);
        // Group resets touch many paths; resync every input.
        if self.schema.find(path).is_some_and(Node::is_group) {
            let all: Vec<String> = self
                .inputs
                .values()
                .filter_map(|(_, role)| role_path(role).map(str::to_owned))
                .collect();
            self.stale.extend(all);
        }
        self.store.reset(path);
        self.errors.clear();
        self.store.save();
        cx.emit(ConfigChanged);
        cx.notify();
    }

    fn support_ok(&self, node: &Node) -> bool {
        match (self.support_filter, node.support) {
            (SupportFilter::All, _) | (_, None) => true,
            (SupportFilter::Working, Some(support)) => support.status != Status::Gap,
            (SupportFilter::Planned, Some(support)) => support.status == Status::Gap,
        }
    }

    fn show(&self, node: &Node, filter: &Filter, inherited: bool) -> bool {
        if node.level > filter.level {
            return false;
        }
        if !node.is_group() {
            return filter.visible_inner(node, inherited) && self.support_ok(node);
        }
        filter
            .children(node, inherited)
            .into_iter()
            .any(|(child, inherited)| self.show(child, filter, inherited))
    }

    // Toolbar ---------------------------------------------------------------------

    fn render_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let search = self.ensure_input(Role::Search, "Search settings", window, cx);
        let mut levels = div().flex().items_center().gap(px(2.0));
        for level in Level::ALL {
            let active = self.level == level;
            levels = levels.child(
                button(
                    format!("cfg-level-{}", level.id()),
                    level.label(),
                    if active { Btn::Selected } else { Btn::Subtle },
                )
                .h(px(24.0))
                .px(px(8.0))
                .py(px(0.0))
                .text_size(px(11.5))
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.level = level;
                    page.store.set_ui("level", json!(level.id()));
                    cx.notify();
                })),
            );
        }
        let mut support = div().flex().items_center().gap(px(2.0));
        for (id, title, which) in [
            ("all", "All", SupportFilter::All),
            ("working", "Working", SupportFilter::Working),
            ("planned", "Planned", SupportFilter::Planned),
        ] {
            let active = self.support_filter == which;
            support = support.child(
                button(
                    format!("cfg-support-{id}"),
                    title,
                    if active { Btn::Selected } else { Btn::Subtle },
                )
                .h(px(24.0))
                .px(px(8.0))
                .py(px(0.0))
                .text_size(px(11.5))
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.support_filter = which;
                    cx.notify();
                })),
            );
        }

        let profiles = self.store.profiles();
        let items: Vec<ChoiceItem> = profiles
            .iter()
            .map(|name| ChoiceItem {
                label: name.clone(),
                value: json!(name),
            })
            .collect();
        let current = json!(self.store.profile());
        let profile_select = self.ensure_select("#profile", items, &current, false, window, cx);
        let name_input = self.ensure_input(Role::ProfileName, "New profile name", window, cx);

        let profile_buttons = div()
            .flex()
            .items_center()
            .gap(px(3.0))
            .child(
                Select::new(&profile_select)
                    .xsmall()
                    .w(px(130.0))
                    .h(px(24.0))
                    .px(px(6.0))
                    .bg(rgb(PANEL_DARK))
                    .border_1()
                    .border_color(rgb(LINE))
                    .rounded(px(1.0))
                    .text_size(px(11.5))
                    .text_color(rgb(TEXT)),
            )
            .child(Input::new(&name_input).xsmall().w(px(130.0)))
            .child(
                button("cfg-profile-new", "New", Btn::Subtle)
                    .h(px(24.0))
                    .px(px(8.0))
                    .py(px(0.0))
                    .text_size(px(11.5))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.profile_action(false, window, cx);
                    })),
            )
            .child(
                button("cfg-profile-copy", "Copy", Btn::Subtle)
                    .h(px(24.0))
                    .px(px(8.0))
                    .py(px(0.0))
                    .text_size(px(11.5))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.profile_action(true, window, cx);
                    })),
            )
            .child(
                button("cfg-profile-delete", "Delete", Btn::Subtle)
                    .h(px(24.0))
                    .px(px(8.0))
                    .py(px(0.0))
                    .text_size(px(11.5))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.delete_current_profile(window, cx);
                    })),
            );

        div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.0))
            .px(px(8.0))
            .py(px(5.0))
            .bg(rgb(PANEL_DARK))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(Input::new(&search).xsmall().w(px(200.0)))
            .child(levels)
            .child(support)
            .child(profile_buttons)
            .into_any_element()
    }

    fn profile_action(&mut self, copy: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.input_text(&Role::ProfileName, cx).trim().to_owned();
        let source = copy.then(|| self.store.profile().to_owned());
        match self.store.create_profile(&name, source.as_deref()) {
            Ok(()) => {
                self.notice = None;
                self.switch_profile(&name, window, cx);
                if let Some((input, _)) = self.inputs.get(&Role::ProfileName.key()) {
                    input.update(cx, |state, cx| state.set_value("", window, cx));
                }
            }
            Err(error) => {
                self.notice = Some(error);
                cx.notify();
            }
        }
    }

    fn delete_current_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.store.profile().to_owned();
        if current == DEFAULT_PROFILE {
            self.notice = Some("The default profile cannot be deleted".into());
            cx.notify();
            return;
        }
        self.switch_profile(DEFAULT_PROFILE, window, cx);
        match self.store.delete_profile(&current) {
            Ok(()) => self.notice = Some(format!("Deleted profile {current}")),
            Err(error) => self.notice = Some(error),
        }
        cx.notify();
    }

    // Left tree -------------------------------------------------------------------

    fn render_tabs(&mut self, filter: &Filter, cx: &mut Context<Self>) -> AnyElement {
        let schema = self.schema.clone();
        let mut tree = div()
            .id("cfg-tabs")
            .w(px(205.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .py(px(4.0))
            .bg(rgb(PANEL_DARK))
            .border_r_1()
            .border_color(rgb(LINE));
        if filter.searching() {
            let active = self.all_results;
            tree = tree.child(
                div()
                    .id("cfg-tab-all-results")
                    .h(px(25.0))
                    .px(px(9.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(rgb(if active { TEXT } else { MUTED }))
                    .bg(rgb(if active { BLUE_TINT } else { PANEL_DARK }))
                    .cursor_pointer()
                    .on_click(cx.listener(|page, _, _, cx| {
                        page.all_results = true;
                        cx.notify();
                    }))
                    .child("All matches"),
            );
        }
        let mut rows = Vec::new();
        for root in &schema.roots {
            if root.is_group() && self.show(root, filter, false) {
                self.tab_rows(&mut rows, root, 0, filter, cx);
            }
        }
        tree.children(rows).into_any_element()
    }

    fn tab_rows(
        &self,
        rows: &mut Vec<AnyElement>,
        node: &Node,
        depth: usize,
        filter: &Filter,
        cx: &mut Context<Self>,
    ) {
        let has_groups = node.children.iter().any(Node::is_group);
        let open = self.tabs_open.contains(&node.path);
        let active = !self.all_results && self.selected == node.path;
        let path = node.path.clone();
        let toggle_path = node.path.clone();
        rows.push(
            div()
                .id(format!("cfg-tab-{}", node.path))
                .h(px(25.0))
                .flex()
                .items_center()
                .pl(px(6.0 + depth as f32 * 12.0))
                .pr(px(6.0))
                .gap(px(3.0))
                .bg(rgb(if active { BLUE_TINT } else { PANEL_DARK }))
                .hover(move |style| style.bg(rgb(if active { BLUE_TINT } else { PANEL_HOVER })))
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    page.select_group(&path);
                    cx.notify();
                }))
                .child(
                    div()
                        .id(format!("cfg-tab-arrow-{}", node.path))
                        .w(px(12.0))
                        .text_size(px(11.0))
                        .text_color(rgb(MUTED))
                        .on_click(cx.listener(move |page, _, _, cx| {
                            cx.stop_propagation();
                            if !page.tabs_open.remove(&toggle_path) {
                                page.tabs_open.insert(toggle_path.clone());
                            }
                            cx.notify();
                        }))
                        .child(if !has_groups {
                            ""
                        } else if open {
                            "▾"
                        } else {
                            "▸"
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgb(if active { TEXT } else { MUTED }))
                        .child(node.label.clone()),
                )
                .into_any_element(),
        );
        if open || filter.searching() {
            for (child, _) in filter.children(node, false) {
                if child.is_group() && self.show(child, filter, false) {
                    self.tab_rows(rows, child, depth + 1, filter, cx);
                }
            }
        }
    }

    // Right pane ------------------------------------------------------------------

    fn render_content(
        &mut self,
        filter: &Filter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let schema = self.schema.clone();
        let mut pane = div()
            .id("cfg-content")
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_y_scrollbar()
            .px(px(10.0))
            .py(px(6.0));

        let roots: Vec<&Node> = if filter.searching() && self.all_results {
            schema.roots.iter().collect()
        } else {
            schema.find(&self.selected).into_iter().collect()
        };
        let mut any = false;
        for root in roots {
            if !self.show(root, filter, false) {
                continue;
            }
            any = true;
            pane = pane.child(self.group_heading(root, true, cx));
            for element in self.render_group_children(root, filter, false, 0, window, cx) {
                pane = pane.child(element);
            }
        }
        if !any {
            pane = pane.child(label(
                "No settings match the current search, visibility level and filter.",
                MUTED,
                12.0,
            ));
        }
        pane.into_any_element()
    }

    fn group_heading(&self, node: &Node, top: bool, cx: &mut Context<Self>) -> AnyElement {
        let folded = self.folded.contains(&node.path);
        let path = node.path.clone();
        let reset_path = node.path.clone();
        let mut row = div()
            .id(format!("cfg-group-{}", node.path))
            .mt(px(if top { 0.0 } else { 8.0 }))
            .h(px(if top { 28.0 } else { 22.0 }))
            .flex()
            .items_center()
            .gap(px(6.0))
            .border_b_1()
            .border_color(rgb(LINE));
        if !top {
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    if !page.folded.remove(&path) {
                        page.folded.insert(path.clone());
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(10.0))
                        .text_size(px(12.0))
                        .text_color(rgb(MUTED))
                        .child(if folded { "▸" } else { "▾" }),
                );
        }
        row = row.child(
            div()
                .text_size(px(if top { 14.0 } else { 12.0 }))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(TEXT))
                .child(node.label.clone()),
        );
        if !node.desc.is_empty() {
            row = row.child(label(&node.desc, FAINT, 11.5));
        }
        row = row.child(div().flex_1()).child(
            div()
                .id(format!("cfg-group-reset-{}", node.path))
                .px(px(6.0))
                .text_size(px(11.0))
                .text_color(rgb(FAINT))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(TEXT)))
                .on_click(cx.listener(move |page, _, _, cx| {
                    cx.stop_propagation();
                    page.reset_path(&reset_path, cx);
                }))
                .child("Reset group"),
        );
        row.into_any_element()
    }

    fn render_group_children(
        &mut self,
        node: &Node,
        filter: &Filter,
        inherited: bool,
        depth: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut out = Vec::new();
        for (child, inherited) in filter.children(node, inherited) {
            if !self.show(child, filter, inherited) {
                continue;
            }
            if child.is_group() {
                out.push(self.group_heading(child, false, cx));
                if !self.folded.contains(&child.path) || filter.searching() {
                    let nested =
                        self.render_group_children(child, filter, inherited, depth + 1, window, cx);
                    out.push(
                        div()
                            .pl(px(14.0))
                            .flex()
                            .flex_col()
                            .children(nested)
                            .into_any_element(),
                    );
                }
            } else {
                out.push(self.render_option(child, window, cx));
            }
        }
        out
    }

    fn render_footer(&self) -> AnyElement {
        let (mut live, mut partial, mut gap) = (0, 0, 0);
        for leaf in self.schema.leaves() {
            match leaf.support.map(|support| support.status) {
                Some(Status::Live) => live += 1,
                Some(Status::Partial) => partial += 1,
                _ => gap += 1,
            }
        }
        let mut footer = div()
            .flex_shrink_0()
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .px(px(8.0))
            .bg(rgb(PANEL_DARK))
            .border_t_1()
            .border_color(rgb(LINE))
            .child(label(
                &format!(
                    "Profile: {}   ·   {live} live   ·   {partial} partial   ·   {gap} planned (see docs/SETTINGS_GAPS.md)",
                    self.store.profile()
                ),
                FAINT,
                11.0,
            ));
        if let Some(notice) = &self.notice {
            footer = footer.child(label(notice, 0xf0b9b9, 11.5));
        }
        footer.into_any_element()
    }

    // Options ---------------------------------------------------------------------

    fn render_option(
        &mut self,
        node: &Node,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(kind) = node.kind.clone() else {
            return div().into_any_element();
        };
        let value = self.store.get(&node.path);
        let wide = matches!(kind, Kind::Table(_));
        let editor = self.render_editor(node, &kind, &value, window, cx);
        let path = node.path.clone();

        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .min_h(px(24.0))
            .py(px(1.0));
        let is_bool = matches!(kind, Kind::Bool);
        if !is_bool {
            row = row.child(field_label(&node.label, if wide { 160.0 } else { 250.0 }));
        }
        row = row.child(editor);
        row = row.child(div().flex_1());
        if let Some(support) = node.support {
            row = row.child(self.support_chip(node, support, cx));
        }
        if !self.store.is_default(&path) {
            let reset = path.clone();
            row = row.child(
                div()
                    .id(format!("cfg-reset-{path}"))
                    .px(px(4.0))
                    .text_size(px(12.0))
                    .text_color(rgb(FAINT))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(TEXT)))
                    .on_click(cx.listener(move |page, _, _, cx| page.reset_path(&reset, cx)))
                    .child("↺"),
            );
        }

        let mut block = div().flex().flex_col().child(row);
        if !node.desc.is_empty() {
            block = block.child(
                div()
                    .pl(px(2.0))
                    .pb(px(2.0))
                    .child(label(&node.desc, FAINT, 11.5)),
            );
        }
        for (key, error) in &self.errors {
            if key == &path
                || key.starts_with(&format!("{path}#"))
                || key.starts_with(&format!("{path}/"))
            {
                block = block.child(label(error, 0xf0b9b9, 11.5));
            }
        }
        if self.info_open.contains(&path)
            && let Some(support) = node.support
        {
            block = block.child(self.support_detail(support));
        }
        block.into_any_element()
    }

    fn support_chip(
        &self,
        node: &Node,
        support: super::super::schema::Support,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (text, bg, fg) = match support.status {
            Status::Live => ("LIVE", GREEN_TINT, GREEN),
            Status::Partial => ("PARTIAL", 0x5e5231, AMBER),
            Status::Gap => ("PLANNED", RED_TINT, 0xd9a0a0),
        };
        let path = node.path.clone();
        div()
            .id(format!("cfg-chip-{}", node.path))
            .h(px(16.0))
            .px(px(6.0))
            .flex()
            .items_center()
            .rounded(px(8.0))
            .bg(rgb(bg))
            .text_size(px(10.0))
            .text_color(rgb(fg))
            .cursor_pointer()
            .on_click(cx.listener(move |page, _, _, cx| {
                if !page.info_open.remove(&path) {
                    page.info_open.insert(path.clone());
                }
                cx.notify();
            }))
            .child(text)
            .into_any_element()
    }

    fn support_detail(&self, support: super::super::schema::Support) -> AnyElement {
        let mut card = div()
            .ml(px(4.0))
            .my(px(2.0))
            .p(px(6.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .bg(rgb(PANEL_DARK))
            .border_1()
            .border_color(rgb(LINE_SOFT))
            .rounded(px(2.0));
        if !support.target.is_empty() {
            card = card.child(label(
                &format!("Runtime target: {}", support.target),
                MUTED,
                11.5,
            ));
        }
        if let Some(gap) = gaps::find(support.gap) {
            let layers = gap
                .layers
                .iter()
                .map(|layer| layer.label())
                .collect::<Vec<_>>()
                .join(" + ");
            card = card
                .child(label(
                    &format!("{} — needs: {layers}", gap.title),
                    TEXT,
                    11.5,
                ))
                .child(label(gap.needs, MUTED, 11.5));
        }
        card.into_any_element()
    }

    fn render_editor(
        &mut self,
        node: &Node,
        kind: &Kind,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = node.path.clone();
        match kind {
            Kind::Bool => {
                self.bool_editor(&path, &node.label, value.as_bool().unwrap_or(false), cx)
            }
            Kind::Int { .. } | Kind::Float { .. } | Kind::Percent { .. } => {
                self.number_editor(&path, kind, value, window, cx)
            }
            Kind::Text | Kind::Condition | Kind::Json => {
                let width = match kind {
                    Kind::Json => 420.0,
                    Kind::Condition => 320.0,
                    _ => 240.0,
                };
                let placeholder = if matches!(kind, Kind::Condition) {
                    "condition"
                } else {
                    ""
                };
                let input = self.ensure_input(Role::Value(path), placeholder, window, cx);
                Input::new(&input).xsmall().w(px(width)).into_any_element()
            }
            Kind::Char => self.key_editor(KeyTarget::Value(path), window, cx),
            Kind::Choice {
                options,
                multi: false,
            } => {
                let items = options
                    .iter()
                    .map(|(label, value)| ChoiceItem {
                        label: label.clone(),
                        value: value.clone(),
                    })
                    .collect();
                self.select_editor(&path, items, value, false, 220.0, window, cx)
            }
            Kind::Choice {
                options,
                multi: true,
            } => self.chips_editor(&path, options, value, cx),
            Kind::Dynamic(Source::Modules) => {
                let mut items: Vec<ChoiceItem> = self
                    .modules
                    .iter()
                    .map(|(id, name)| ChoiceItem {
                        label: name.clone(),
                        value: json!(id),
                    })
                    .collect();
                if let Some(current) = value.as_str()
                    && !items.iter().any(|item| item.value == *value)
                {
                    items.push(ChoiceItem {
                        label: current.to_owned(),
                        value: value.clone(),
                    });
                }
                self.select_editor(&path, items, value, false, 260.0, window, cx)
            }
            Kind::Dynamic(Source::Maps) => {
                let selected = resolved_map_value(&self.maps, value);
                let items = map_choices(&self.maps, value)
                    .into_iter()
                    .map(|choice| ChoiceItem {
                        label: choice.label,
                        value: choice.value,
                    })
                    .collect();
                self.select_editor(&path, items, &selected, true, 280.0, window, cx)
            }
            Kind::Tag(default) => {
                let none = match default {
                    TagDefault::None => "No tag",
                    TagDefault::All => "Everyone",
                };
                let mut items = vec![ChoiceItem {
                    label: none.into(),
                    value: Value::Null,
                }];
                items.extend(self.tags.iter().map(|tag| ChoiceItem {
                    label: tag.clone(),
                    value: json!(tag),
                }));
                if let Some(current) = value.as_str()
                    && !self.tags.iter().any(|tag| tag == current)
                {
                    items.push(ChoiceItem {
                        label: current.to_owned(),
                        value: value.clone(),
                    });
                }
                self.select_editor(&path, items, value, false, 200.0, window, cx)
            }
            Kind::Range => {
                let min = self.ensure_input(
                    Role::Range {
                        path: path.clone(),
                        max: false,
                    },
                    "min",
                    window,
                    cx,
                );
                let max = self.ensure_input(Role::Range { path, max: true }, "max", window, cx);
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(Input::new(&min).xsmall().w(px(56.0)))
                    .child(label("% –", MUTED, 12.0))
                    .child(Input::new(&max).xsmall().w(px(56.0)))
                    .child(label("%", MUTED, 12.0))
                    .into_any_element()
            }
            Kind::Ship => {
                let config = value["config"].as_i64().unwrap_or(1);
                let mut row = div().flex().items_center().gap(px(3.0));
                for which in [1_i64, 2] {
                    let key = path.clone();
                    row = row.child(
                        button(
                            format!("cfg-ship-{path}-{which}"),
                            format!("Config {which}"),
                            if config == which {
                                Btn::Selected
                            } else {
                                Btn::Subtle
                            },
                        )
                        .h(px(23.0))
                        .px(px(8.0))
                        .py(px(0.0))
                        .text_size(px(11.5))
                        .on_click(cx.listener(move |page, _, _, cx| {
                            let mut ship = page.store.get(&key);
                            ship["config"] = json!(which);
                            page.set(&key, ship, cx);
                        })),
                    );
                }
                let formation = self.key_editor(KeyTarget::Formation(path), window, cx);
                row.child(label("Formation", MUTED, 12.0))
                    .child(formation)
                    .into_any_element()
            }
            Kind::Color { .. } => {
                let swatch = parse_hex(value.as_str().unwrap_or("#000000"));
                let input = self.ensure_input(Role::Value(path), "#rrggbb", window, cx);
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .size(px(16.0))
                            .rounded(px(2.0))
                            .border_1()
                            .border_color(rgb(LINE))
                            .bg(rgb(swatch)),
                    )
                    .child(Input::new(&input).xsmall().w(px(96.0)))
                    .into_any_element()
            }
            Kind::Table(spec) => self.table_editor(&path, spec, value, window, cx),
        }
    }

    /// the reference implementation's key-capture box: click it, press the key to bind. Backspace or
    /// Delete clears, Escape (or clicking away) cancels. Keys without a
    /// character are stored by name (`F1`, `Enter`, `Up`, ...).
    fn key_editor(
        &mut self,
        target: KeyTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = target.id();
        if !self.key_focus.contains_key(&id) {
            let handle = cx.focus_handle();
            // Repaint when focus enters or leaves so the prompt never goes stale.
            self.subscriptions
                .push(cx.on_focus(&handle, window, |_, _, cx| cx.notify()));
            self.subscriptions
                .push(cx.on_blur(&handle, window, |_, _, cx| cx.notify()));
            self.key_focus.insert(id.clone(), handle);
        }
        let handle = self.key_focus[&id].clone();
        let capturing = handle.is_focused(window);
        let current = self.key_current(&target);

        let text = if capturing {
            "Press a key…".to_owned()
        } else {
            current
                .as_deref()
                .map(super::super::keys::display)
                .unwrap_or_else(|| "—".to_owned())
        };
        let (press_target, clear_target, focus_handle) = (target.clone(), target, handle.clone());
        let mut row = div().flex().items_center().gap(px(3.0)).child(
            div()
                .id(format!("cfg-key-{id}"))
                .track_focus(&handle)
                .w(px(96.0))
                .h(px(23.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .bg(rgb(PANEL_DARK))
                .border_1()
                .border_color(rgb(if capturing { BLUE } else { LINE }))
                .rounded(px(1.0))
                .cursor_pointer()
                .text_size(px(11.5))
                .text_color(rgb(if capturing {
                    BLUE
                } else if current.is_some() {
                    TEXT
                } else {
                    FAINT
                }))
                .on_click(cx.listener(move |_, _, window, cx| {
                    window.focus(&focus_handle, cx);
                    cx.notify();
                }))
                .on_key_down(cx.listener(
                    move |page, event: &gpui_kit::KeyDownEvent, window, cx| {
                        use super::super::keys::{Press, interpret};
                        cx.stop_propagation();
                        match interpret(&event.keystroke.key, event.keystroke.key_char.as_deref()) {
                            Press::Key(key) => {
                                page.key_picked(&press_target, Some(key), cx);
                                window.blur(cx);
                            }
                            Press::Clear => {
                                page.key_picked(&press_target, None, cx);
                                window.blur(cx);
                            }
                            Press::Cancel => window.blur(cx),
                            Press::Ignore => {}
                        }
                        cx.notify();
                    },
                ))
                .child(text),
        );
        if current.is_some() && !capturing {
            row = row.child(
                div()
                    .id(format!("cfg-key-clear-{id}"))
                    .px(px(4.0))
                    .text_size(px(11.0))
                    .text_color(rgb(FAINT))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(0xf0b9b9)))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.key_picked(&clear_target, None, cx);
                    }))
                    .child("✕"),
            );
        }
        row.into_any_element()
    }

    fn bool_editor(
        &mut self,
        path: &str,
        text: &str,
        checked: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = cx.entity().downgrade();
        let key = path.to_owned();
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                SemanticCheckbox::new(format!("cfg-{path}"))
                    .checked(checked)
                    .accessibility_label(text.to_owned())
                    .size(px(13.0))
                    .on_change(move |next, _, _, cx| {
                        let _ = page.update(cx, |page, cx| {
                            page.set(&key, json!(matches!(next, CheckboxState::Checked)), cx);
                        });
                    })
                    .child(indicator(checked)),
            )
            .child(label(text, TEXT, 12.0))
            .into_any_element()
    }

    fn number_editor(
        &mut self,
        path: &str,
        kind: &Kind,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut row = div().flex().items_center().gap(px(4.0));
        let current = value.as_f64().unwrap_or(0.0);
        let disabled = match kind {
            Kind::Int {
                disabled: Some(d), ..
            } => Some(d.clone()),
            _ => None,
        };
        let is_off = disabled.as_ref().is_some_and(|d| current == d.value);
        if let Some(d) = &disabled {
            let page = cx.entity().downgrade();
            let key = path.to_owned();
            let (off, def) = (d.value, d.def);
            let checked = !is_off;
            row = row.child(
                SemanticCheckbox::new(format!("cfg-enable-{path}"))
                    .checked(checked)
                    .accessibility_label("Enabled".to_owned())
                    .size(px(13.0))
                    .on_change(move |next, _, _, cx| {
                        let _ = page.update(cx, |page, cx| {
                            let target = if matches!(next, CheckboxState::Checked) {
                                def
                            } else {
                                off
                            };
                            page.set_external(&key, json!(target), cx);
                        });
                    })
                    .child(indicator(checked)),
            );
        }
        if is_off {
            return row.child(label("disabled", FAINT, 12.0)).into_any_element();
        }
        let step = step_of(kind).unwrap_or(1.0);
        let (dec_key, inc_key) = (path.to_owned(), path.to_owned());
        let dec = cx.listener(move |page, _, _, cx| {
            let now = page.store.get(&dec_key).as_f64().unwrap_or(0.0);
            let next = ((now - step) * 10_000.0).round() / 10_000.0;
            page.set_external(&dec_key, json!(next), cx);
        });
        let inc = cx.listener(move |page, _, _, cx| {
            let now = page.store.get(&inc_key).as_f64().unwrap_or(0.0);
            let next = ((now + step) * 10_000.0).round() / 10_000.0;
            page.set_external(&inc_key, json!(next), cx);
        });
        let input = self.ensure_input(Role::Value(path.to_owned()), "", window, cx);
        row = row
            .child(
                button(format!("cfg-dec-{path}"), "−", Btn::Subtle)
                    .size(px(22.0))
                    .px(px(0.0))
                    .py(px(0.0))
                    .on_click(dec),
            )
            .child(Input::new(&input).xsmall().w(px(78.0)))
            .child(
                button(format!("cfg-inc-{path}"), "+", Btn::Subtle)
                    .size(px(22.0))
                    .px(px(0.0))
                    .py(px(0.0))
                    .on_click(inc),
            );
        if matches!(kind, Kind::Percent { .. }) {
            row = row.child(label("%", MUTED, 12.0));
        }
        row.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn select_editor(
        &mut self,
        path: &str,
        items: Vec<ChoiceItem>,
        selected: &Value,
        searchable: bool,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let select = self.ensure_select(path, items, selected, searchable, window, cx);
        Select::new(&select)
            .xsmall()
            .w(px(width))
            .h(px(23.0))
            .px(px(6.0))
            .bg(rgb(PANEL_DARK))
            .border_1()
            .border_color(rgb(LINE))
            .rounded(px(1.0))
            .text_size(px(11.5))
            .text_color(rgb(TEXT))
            .into_any_element()
    }

    fn chips_editor(
        &mut self,
        path: &str,
        options: &[(String, Value)],
        value: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chosen = value.as_array().cloned().unwrap_or_default();
        let mut chips = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(3.0))
            .max_w(px(560.0));
        for (index, (text, option)) in options.iter().enumerate() {
            let selected = chosen.contains(option);
            let key = path.to_owned();
            let option = option.clone();
            chips = chips.child(
                div()
                    .id(format!("cfg-chip-{path}-{index}"))
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
                        let mut now = page.store.get(&key).as_array().cloned().unwrap_or_default();
                        if let Some(at) = now.iter().position(|item| *item == option) {
                            now.remove(at);
                        } else {
                            now.push(option.clone());
                        }
                        page.set(&key, Value::Array(now), cx);
                    }))
                    .child(text.clone()),
            );
        }
        chips.into_any_element()
    }

    // Tables ----------------------------------------------------------------------

    fn table_editor(
        &mut self,
        path: &str,
        spec: &TableSpec,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const MAX_ROWS: usize = 200;
        let rows = value.as_object().cloned().unwrap_or_default();
        let search = self.ensure_input(
            Role::NewRow(path.to_owned()),
            "Filter or new name",
            window,
            cx,
        );
        let query = self
            .input_text(&Role::NewRow(path.to_owned()), cx)
            .trim()
            .to_lowercase();

        let add_key = path.to_owned();
        let defaults = row_defaults(spec);
        let mut controls = div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(Input::new(&search).xsmall().w(px(200.0)));
        if spec.editable_rows {
            controls = controls.child(
                button(format!("cfg-add-{path}"), "Add", Btn::Subtle)
                    .h(px(23.0))
                    .px(px(8.0))
                    .py(px(0.0))
                    .text_size(px(11.5))
                    .on_click(cx.listener(move |page, _, window, cx| {
                        let name = page
                            .input_text(&Role::NewRow(add_key.clone()), cx)
                            .trim()
                            .to_owned();
                        if name.is_empty() {
                            return;
                        }
                        let mut table = page.store.get(&add_key);
                        if let Some(rows) = table.as_object_mut() {
                            rows.entry(name).or_insert_with(|| defaults.clone());
                        }
                        page.set(&add_key, table, cx);
                        if let Some((input, _)) =
                            page.inputs.get(&Role::NewRow(add_key.clone()).key())
                        {
                            input.update(cx, |state, cx| state.set_value("", window, cx));
                        }
                    })),
            );
        }
        controls = controls.child(label(&format!("{} rows", rows.len()), FAINT, 11.5));

        let mut header = div()
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(6.0))
            .bg(rgb(PANEL_DARK))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(
                div()
                    .w(px(190.0))
                    .flex_shrink_0()
                    .child(label(&spec.key_label, MUTED, 12.0)),
            );
        for column in &spec.columns {
            header = header.child(
                div()
                    .w(px(column_width(&column.kind)))
                    .flex_shrink_0()
                    .child(label(&column.label, MUTED, 12.0)),
            );
        }

        let mut block = div().flex().flex_col().gap(px(3.0)).child(controls);
        if !spec.key_desc.is_empty() {
            block = block.child(label(&spec.key_desc, FAINT, 11.5));
        }
        block = block.child(header);
        let mut shown = 0;
        for (name, row) in &rows {
            if !query.is_empty() && !name.to_lowercase().contains(&query) {
                continue;
            }
            if shown >= MAX_ROWS {
                block = block.child(label("Refine the filter to see more rows.", FAINT, 11.5));
                break;
            }
            shown += 1;
            block = block.child(self.table_row(path, spec, name, row, window, cx));
        }
        if rows.is_empty() {
            block = block.child(label(
                if spec.discover.is_some() {
                    "None yet — names show up here as they are seen in game."
                } else {
                    "No entries."
                },
                FAINT,
                11.5,
            ));
        }
        block.into_any_element()
    }

    fn table_row(
        &mut self,
        path: &str,
        spec: &TableSpec,
        name: &str,
        row: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut line = div()
            .min_h(px(24.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(6.0))
            .border_b_1()
            .border_color(rgb(LINE_SOFT))
            .child(
                div()
                    .w(px(190.0))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(label(name, TEXT, 12.0)),
            );
        let mut expanded: Option<AnyElement> = None;
        for column in &spec.columns {
            let cell = &row[column.key.as_str()];
            let width = column_width(&column.kind);
            let element: AnyElement = match &column.kind {
                Kind::Bool => {
                    let page = cx.entity().downgrade();
                    let (table_path, row_name, col) =
                        (path.to_owned(), name.to_owned(), column.key.clone());
                    let checked = cell.as_bool().unwrap_or(false);
                    SemanticCheckbox::new(format!("cfg-cell-{path}-{name}-{}", column.key))
                        .checked(checked)
                        .accessibility_label(column.label.clone())
                        .size(px(13.0))
                        .on_change(move |next, _, _, cx| {
                            let _ = page.update(cx, |page, cx| {
                                let mut table = page.store.get(&table_path);
                                table[row_name.as_str()][col.as_str()] =
                                    json!(matches!(next, CheckboxState::Checked));
                                page.set(&table_path, table, cx);
                            });
                        })
                        .child(indicator(checked))
                        .into_any_element()
                }
                Kind::Choice {
                    options,
                    multi: true,
                } => {
                    let cell_key = format!("{path}/{name}/{}", column.key);
                    let open = self.cells_open.contains(&cell_key);
                    let summary = cell
                        .as_array()
                        .map(|items| {
                            options
                                .iter()
                                .filter(|(_, option)| items.contains(option))
                                .map(|(label, _)| short_flag(label))
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .unwrap_or_default();
                    if open {
                        let (table_path, row_name, col) =
                            (path.to_owned(), name.to_owned(), column.key.clone());
                        let current = cell.as_array().cloned().unwrap_or_default();
                        let mut chips = div().flex().flex_wrap().gap(px(3.0)).py(px(3.0));
                        for (index, (text, option)) in options.iter().enumerate() {
                            let selected = current.contains(option);
                            let (table_path, row_name, col, option) = (
                                table_path.clone(),
                                row_name.clone(),
                                col.clone(),
                                option.clone(),
                            );
                            chips = chips.child(
                                div()
                                    .id(format!("cfg-flag-{path}-{name}-{index}"))
                                    .h(px(20.0))
                                    .px(px(6.0))
                                    .flex()
                                    .items_center()
                                    .bg(rgb(if selected { BLUE } else { PANEL_DARK }))
                                    .border_1()
                                    .border_color(rgb(if selected { 0x6d9bd2 } else { LINE }))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .text_color(rgb(if selected { 0xffffff } else { TEXT }))
                                    .on_click(cx.listener(move |page, _, _, cx| {
                                        let mut table = page.store.get(&table_path);
                                        let mut now = table[row_name.as_str()][col.as_str()]
                                            .as_array()
                                            .cloned()
                                            .unwrap_or_default();
                                        if let Some(at) =
                                            now.iter().position(|item| *item == option)
                                        {
                                            now.remove(at);
                                        } else {
                                            now.push(option.clone());
                                        }
                                        table[row_name.as_str()][col.as_str()] = Value::Array(now);
                                        page.set(&table_path, table, cx);
                                    }))
                                    .child(text.clone()),
                            );
                        }
                        expanded = Some(chips.into_any_element());
                    }
                    let toggle = cell_key.clone();
                    div()
                        .id(format!("cfg-cell-toggle-{cell_key}"))
                        .h(px(21.0))
                        .px(px(6.0))
                        .flex()
                        .items_center()
                        .border_1()
                        .border_color(rgb(LINE))
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .text_color(rgb(TEXT))
                        .on_click(cx.listener(move |page, _, _, cx| {
                            if !page.cells_open.remove(&toggle) {
                                page.cells_open.insert(toggle.clone());
                            }
                            cx.notify();
                        }))
                        .child(if summary.is_empty() {
                            "—".to_owned()
                        } else {
                            summary
                        })
                        .into_any_element()
                }
                Kind::Char => self.key_editor(
                    KeyTarget::Cell {
                        path: path.to_owned(),
                        row: name.to_owned(),
                        col: column.key.clone(),
                    },
                    window,
                    cx,
                ),
                _ => {
                    let input = self.ensure_input(
                        Role::Cell {
                            path: path.to_owned(),
                            row: name.to_owned(),
                            col: column.key.clone(),
                        },
                        "",
                        window,
                        cx,
                    );
                    Input::new(&input)
                        .xsmall()
                        .w(px(width - 8.0))
                        .into_any_element()
                }
            };
            line = line.child(div().w(px(width)).flex_shrink_0().child(element));
        }
        if spec.editable_rows {
            let (table_path, row_name) = (path.to_owned(), name.to_owned());
            line = line.child(
                div()
                    .id(format!("cfg-row-remove-{path}-{name}"))
                    .px(px(6.0))
                    .text_size(px(12.0))
                    .text_color(rgb(FAINT))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(0xf0b9b9)))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        let mut table = page.store.get(&table_path);
                        if let Some(rows) = table.as_object_mut() {
                            rows.remove(&row_name);
                        }
                        page.set(&table_path, table, cx);
                    }))
                    .child("✕"),
            );
        }
        let mut block = div().flex().flex_col().child(line);
        if let Some(extra) = expanded {
            block = block.child(div().pl(px(8.0)).child(extra));
        }
        block.into_any_element()
    }
}

fn column_width(kind: &Kind) -> f32 {
    match kind {
        Kind::Bool => 76.0,
        Kind::Int { .. } | Kind::Float { .. } | Kind::Percent { .. } => 88.0,
        Kind::Char => 110.0,
        Kind::Choice { .. } => 150.0,
        _ => 170.0,
    }
}

/// Two-letter abbreviation for the extra-flag summary (`NC`, `IO`, ...).
fn short_flag(label: &str) -> String {
    let parts: Vec<&str> = label.split_whitespace().collect();
    match parts.as_slice() {
        [] => String::new(),
        [single] => single.chars().take(2).collect::<String>().to_uppercase(),
        [first, .., last] => format!(
            "{}{}",
            first.chars().next().unwrap_or(' '),
            last.chars().next().unwrap_or(' ')
        )
        .to_uppercase(),
    }
}
