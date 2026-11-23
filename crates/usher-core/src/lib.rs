//! Pure domain for usher; no I/O, no clock. Filled in M2.
#![forbid(unsafe_code)]

mod commands;
mod desired;
mod guard;
pub mod identity;
mod model;
mod reason;
mod reconcile;
mod remember;
mod seed;
mod state;
pub use commands::*;
pub use desired::*;
pub use guard::*;
pub use identity::Merge;
#[doc(hidden)]
pub use identity::merge_aliases;
pub use model::*;
pub use reason::*;
pub use reconcile::*;
#[doc(hidden)]
pub use remember::remember;
pub use seed::seed;
pub use state::*;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds_as_a_workspace_member() {
        assert_eq!(env!("CARGO_PKG_NAME"), "usher-core");
    }
}
