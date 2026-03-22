use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::subclass::prelude::*;

#[derive(Default)]
pub struct StagePuppetImp {}

#[glib::object_subclass]
impl ObjectSubclass for StagePuppetImp {
    const NAME: &'static str = "NGTStagePuppet";
    type Type = StagePuppet;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-stageborder");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for StagePuppetImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for StagePuppetImp {}

impl ScrollableImpl for StagePuppetImp {}

glib::wrapper! {
    pub struct StagePuppet(ObjectSubclass<StagePuppetImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl StagePuppet {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
