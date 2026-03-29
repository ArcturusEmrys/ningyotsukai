//! Gizmos are custom widgets primarily intended to expose graphical concepts
//! to GTK in a way it can understand.
//!
//! Since GTK4, they've been cutting out ways to create non-widget graphics;
//! so the easiest way to do basically anything is to create a widget, style
//! it with CSS, and attach event handlers to those. "Everything is a widget."
//! It's a very "HTML `<div>` soup"-y idea.
//!
//! The name "gizmo" is in relation to GtkGizmo, a private class in GTK that
//! core widgets use to create graphics with.

mod border;
mod dragsel;
mod origin;
mod puppet;
mod selection;

pub use border::StageBorderGizmo;
pub use dragsel::DragSelectGizmo;
pub use puppet::PuppetBoundsGizmo;
pub use selection::PuppetSelectionGizmo;
