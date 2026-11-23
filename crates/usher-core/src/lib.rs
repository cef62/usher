//! Pure domain for usher: no I/O, no clock, no logging.
//!
//! M3 contract. The engine owns [`State`] (persisted) and [`Runtime`] (in memory).
//! Per coalesced event it passes a [`Snapshot`] to [`reconcile`], executes the
//! returned [`Outcome`]`.actions` (`SetDefault`, `PauseAuto`, `AliasMerged` log,
//! `Persist`) and logs `Outcome.reasons`. UI commands go through [`apply`]
//! followed by [`reconcile`]. [`panel_state`] renders the [`PanelState`] view
//! model. [`seed`] builds the first-launch state; [`from_json`] and [`to_json`]
//! (de)serialise it.
#![forbid(unsafe_code)]

mod commands;
mod desired;
mod guard;
pub mod identity;
mod model;
mod panel;
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
pub use panel::*;
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
