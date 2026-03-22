use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/panels/frame.ui")]
pub struct PanelFrameImp {
    #[template_child]
    handle: TemplateChild<gtk4::Label>,

    #[template_child]
    contents: TemplateChild<gtk4::Frame>,
}

#[glib::object_subclass]
impl ObjectSubclass for PanelFrameImp {
    const NAME: &'static str = "NGTPanelFrame";
    type Type = PanelFrame;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.set_css_name("ningyo-paneldock");
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for PanelFrameImp {
    fn constructed(&self) {
        self.parent_constructed();

        let drag_source = gtk4::DragSource::new();

        let drag_source_prepare_self = self.obj().clone();
        drag_source.connect_prepare(move |_, _, _| {
            let value = glib::Value::from(drag_source_prepare_self.clone());
            Some(gdk4::ContentProvider::for_value(&value))
        });

        let drag_source_begin_self = self.obj().clone();
        drag_source.connect_drag_begin(move |source, _| {
            let preview = gtk4::WidgetPaintable::new(Some(&drag_source_begin_self));
            source.set_icon(Some(&preview), 0, 0);
        });

        self.handle.add_controller(drag_source);
    }
}

impl WidgetImpl for PanelFrameImp {}

impl BoxImpl for PanelFrameImp {}

impl BuildableImpl for PanelFrameImp {
    fn add_child(&self, builder: &gtk4::Builder, object: &glib::Object, name: Option<&str>) {
        if let Some(widget) = object.downcast_ref::<gtk4::Widget>() {
            self.contents.set_child(Some(widget))
        } else {
            self.parent_add_child(builder, object, name)
        }
    }
}

glib::wrapper! {
    pub struct PanelFrame(ObjectSubclass<PanelFrameImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl PanelFrame {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
