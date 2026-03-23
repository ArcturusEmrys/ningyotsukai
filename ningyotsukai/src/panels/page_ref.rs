use glib;

use crate::panels::frame::PanelFrame;
use gtk4::StackPage;

#[derive(Clone, glib::Boxed)]
#[boxed_type(name = "NGTPanelPageRef")]
pub struct PageRef {
    pub frame: PanelFrame,
    pub page: StackPage,
}
