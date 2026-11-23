//! Pure domain for usher; no I/O, no clock. Filled in M2.
#![forbid(unsafe_code)]

mod desired;
mod guard;
pub mod identity;
mod model;
mod reason;
mod state;
pub use desired::*;
pub use guard::*;
pub use model::*;
pub use reason::*;
pub use state::*;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds_as_a_workspace_member() {
        assert_eq!(env!("CARGO_PKG_NAME"), "usher-core");
    }
}
