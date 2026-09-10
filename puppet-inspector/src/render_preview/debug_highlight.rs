use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct DebugHighlightImp {}

#[glib::object_subclass]
impl ObjectSubclass for DebugHighlightImp {
    const NAME: &'static str = "PIDebugHighlight";
    type Type = DebugHighlight;
    type ParentType = gtk4::Widget;
}

impl ObjectImpl for DebugHighlightImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for DebugHighlightImp {}

glib::wrapper! {
    pub struct DebugHighlight(ObjectSubclass<DebugHighlightImp>)
        @extends gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl DebugHighlight {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
