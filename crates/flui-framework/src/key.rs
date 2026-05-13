//! Widget identity keys — re-exported from the K02 [`flui_core`] substrate.
//!
//! In the Framework tier, identity is expressed by three intent types:
//!
//! - [`Key::local()`] — **source-location identity.** `#[track_caller]` —
//!   the key value comes from the call site. Parent-scoped and
//!   disambiguated by sibling occurrence in the parent namespace. NOT
//!   reorder-stable: if the parent reorders its children, local keys
//!   shift accordingly. Use for one-shot widgets that are not part of a
//!   sequence the user can reorder.
//! - [`Key::value()`] — **value identity.** Constructed from a
//!   reorder-stable value (usize / String / etc., via [`ValueKey`]). Use
//!   for list items, tabs, anything where the user identifier survives
//!   reordering.
//! - [`Key::global()`] — **cross-tree global identity.** Holds a
//!   [`GlobalKey`] handle that uniquely identifies an element regardless
//!   of where it appears in the tree. Cross-tree reachability /
//!   move-state semantics arrive in SF02 / SF05; SF01 publishes only the
//!   identity type itself.
//!
//! [`flui_core`]'s engine-level path-segment types `ElementId` and
//! `LocalElementId` are intentionally NOT re-exported here. Framework
//! users speak [`Key`], not raw engine IDs. The K02-blessed conversion
//! [`ValueKey::into_element_id`] remains available for advanced
//! consumers who need to bridge into engine internals.
//!
//! # K91 cross-track contract
//!
//! Today `flui_core::Key`, `ValueKey`, and `GlobalKey` reach the
//! `flui_core` crate root via the `pub use element::*;` glob at
//! `crates/flui-core/src/lib.rs`. When K91 replaces that glob with
//! explicit re-exports, the new list MUST preserve crate-root
//! visibility of `Key`, `ValueKey`, `GlobalKey` — otherwise this
//! module fails to compile. The SF01 design spec records this as a
//! binding cross-track requirement on K91.
//!
//! # Examples
//!
//! ```
//! use flui_framework::Key;
//!
//! // Source-location identity (default for one-shot widgets):
//! let local = Key::local();
//!
//! // Value identity (reorder-stable, for list items):
//! let by_index = Key::value(42usize);
//! let by_name = Key::value(String::from("first-item"));
//!
//! // Global identity (cross-tree handles):
//! let global = Key::global("app-root");
//! # let _ = (local, by_index, by_name, global);
//! ```

pub use flui_core::{GlobalKey, Key, ValueKey};
