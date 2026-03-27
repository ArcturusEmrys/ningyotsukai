//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct PuppetBoundsGizmoImp {}

#[glib::object_subclass]
impl ObjectSubclass for PuppetBoundsGizmoImp {
    const NAME: &'static str = "NGTPuppetBoundsGizmo";
    type Type = PuppetBoundsGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-puppet");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for PuppetBoundsGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj().set_size_request(0, 0);
    }
}

impl WidgetImpl for PuppetBoundsGizmoImp {}

impl ScrollableImpl for PuppetBoundsGizmoImp {}

glib::wrapper! {
    pub struct PuppetBoundsGizmo(ObjectSubclass<PuppetBoundsGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl PuppetBoundsGizmo {
    pub fn new() -> Self {
        let gizmo: Self = glib::Object::builder().build();

        gizmo.set_cursor(gdk4::Cursor::from_name("grab", None).as_ref());

        gizmo
    }
}
