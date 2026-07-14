//! Fixture crate root for module-graph tests.
//!
//! Exercises external file modules, `mod.rs`-style modules, nested external
//! modules, `#[path]` resolution, restricted visibility, and inline modules
//! with nested inline children.

pub mod external_file;
mod nested;
pub(crate) mod restricted;
#[path = "custom/aliased.rs"]
mod aliased;
mod inline_host {
    //! Inline host module declaring a nested inline child.
    pub mod inner {
        const INNER_MARKER: u8 = 0;
    }
}
