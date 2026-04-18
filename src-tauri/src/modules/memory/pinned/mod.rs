//! Pinned-memory subsystem (Phase 8A.9 / T-F1).
//!
//! See [`store`] for the [`PinnedStore`] trait + the SQLite-backed
//! [`SqlitePinnedStore`] implementation, and [`types`] for the public
//! [`PinnedItem`] / [`PinScope`] / [`PinSource`] data shapes.
//!
//! Wiring overview:
//! - `main.rs::run` constructs a [`SqlitePinnedStore`] (or a
//!   [`NullPinnedStore`] fallback) and stores it on
//!   `AppState.pinned_store`.
//! - 8A.10 will wire the `pin_memory` / `unpin_memory` tools through
//!   that field.
//! - 8A.11 will add the system-prompt injector that reads
//!   [`PinnedStore::list_all_for_prompt`].
//! - 8A.12 will add the PinnedMemoryEditor UI.

pub mod store;
pub mod types;

pub use store::{
    NullPinnedStore, PinnedStore, SqlitePinnedStore, MAX_PINS_PER_SCOPE, MAX_PIN_CONTENT_CHARS,
};
pub use types::{PinScope, PinSource, PinnedItem};
