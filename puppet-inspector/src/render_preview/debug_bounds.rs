use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct DebugBoundsImp {}

#[glib::object_subclass]
impl ObjectSubclass for DebugBoundsImp {
    const NAME: &'static str = "PIDebugBounds";
    type Type = DebugBounds;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("pi-debug-bounds");
    }
}

impl ObjectImpl for DebugBoundsImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for DebugBoundsImp {}

glib::wrapper! {
    pub struct DebugBounds(ObjectSubclass<DebugBoundsImp>)
        @extends gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl DebugBounds {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
