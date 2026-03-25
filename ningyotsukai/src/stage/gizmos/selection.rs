//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct PuppetSelectionGizmoImp {}

#[glib::object_subclass]
impl ObjectSubclass for PuppetSelectionGizmoImp {
    const NAME: &'static str = "NGTPuppetSelectionGizmo";
    type Type = PuppetSelectionGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-selection");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for PuppetSelectionGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for PuppetSelectionGizmoImp {}

impl ScrollableImpl for PuppetSelectionGizmoImp {}

glib::wrapper! {
    pub struct PuppetSelectionGizmo(ObjectSubclass<PuppetSelectionGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl PuppetSelectionGizmo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
