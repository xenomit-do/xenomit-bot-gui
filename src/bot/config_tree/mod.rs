//! Legacy-style settings tree: schema, values, profiles, runtime bridge and
//! the GPUI page. See `docs/SETTINGS_TREE.md` and `docs/SETTINGS_GAPS.md`.
mod bridge;
mod legacy;
mod edit;
mod filter;
mod gaps;
mod keys;
mod page;
mod schema;
mod store;
#[cfg(test)]
mod tests;

pub(crate) use page::{ConfigChanged, ConfigTreePage};
