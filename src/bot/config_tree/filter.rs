//! Search and visibility filtering, the Rust twin of DarkBot's `TreeFilter`.
use super::schema::{Level, Node};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Filter {
    pub level: Level,
    /// Lowercased search text; empty means no search.
    pub query: String,
}

impl Filter {
    pub(crate) fn new(level: Level, query: &str) -> Self {
        Self {
            level,
            query: query.trim().to_lowercase(),
        }
    }

    pub(crate) fn searching(&self) -> bool {
        !self.query.is_empty()
    }

    fn text_matches(&self, node: &Node) -> bool {
        node.label.to_lowercase().contains(&self.query)
            || node.desc.to_lowercase().contains(&self.query)
            || node.path.to_lowercase().contains(&self.query)
    }

    /// Whether `node` should be shown. A group whose own name matches the
    /// search shows its whole subtree (`inherited`); otherwise it is shown
    /// when any descendant is.
    #[cfg(test)]
    pub(crate) fn visible(&self, node: &Node) -> bool {
        self.visible_inner(node, false)
    }

    pub(crate) fn visible_inner(&self, node: &Node, inherited: bool) -> bool {
        if node.level > self.level {
            return false;
        }
        let own = !self.searching() || inherited || self.text_matches(node);
        if !node.is_group() {
            return own;
        }
        node.children
            .iter()
            .any(|child| self.visible_inner(child, own && self.searching()))
    }

    /// Children of `node` that pass the filter.
    pub(crate) fn children<'a>(&self, node: &'a Node, inherited: bool) -> Vec<(&'a Node, bool)> {
        let own = !self.searching() || inherited || self.text_matches(node);
        let pass_down = own && self.searching();
        node.children
            .iter()
            .filter(|child| self.visible_inner(child, pass_down))
            .map(|child| (child, pass_down))
            .collect()
    }
}
