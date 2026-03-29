//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct DragSelectGizmoImp {}

#[glib::object_subclass]
impl ObjectSubclass for DragSelectGizmoImp {
    const NAME: &'static str = "NGTDragSelectGizmo";
    type Type = DragSelectGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-dragselect");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for DragSelectGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj().set_size_request(0, 0);
    }
}

impl WidgetImpl for DragSelectGizmoImp {}

impl ScrollableImpl for DragSelectGizmoImp {}

glib::wrapper! {
    pub struct DragSelectGizmo(ObjectSubclass<DragSelectGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl DragSelectGizmo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
