//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct StageBorderGizmoImp {}

#[glib::object_subclass]
impl ObjectSubclass for StageBorderGizmoImp {
    const NAME: &'static str = "NGTStageBorderGizmo";
    type Type = StageBorderGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-stageborder");
    }

    fn instance_init(obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for StageBorderGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for StageBorderGizmoImp {}

impl ScrollableImpl for StageBorderGizmoImp {}

glib::wrapper! {
    pub struct StageBorderGizmo(ObjectSubclass<StageBorderGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl StageBorderGizmo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
