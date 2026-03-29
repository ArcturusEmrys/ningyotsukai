use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct PuppetOriginGizmoImp {}

#[glib::object_subclass]
impl ObjectSubclass for PuppetOriginGizmoImp {
    const NAME: &'static str = "NGTPuppetOriginGizmo";
    type Type = PuppetOriginGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-origin");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for PuppetOriginGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for PuppetOriginGizmoImp {}

impl ScrollableImpl for PuppetOriginGizmoImp {}

glib::wrapper! {
    pub struct PuppetOriginGizmo(ObjectSubclass<PuppetOriginGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl PuppetOriginGizmo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
