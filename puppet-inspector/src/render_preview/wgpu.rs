use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::render_preview::wgpu_inner::MyWgpuArea;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/render_preview/wgpu.ui")]
pub struct InoxWgpuPreviewImp {
    #[template_child]
    wgpu_view: TemplateChild<MyWgpuArea>,
    #[template_child]
    error_view: TemplateChild<gtk4::Frame>,
    #[template_child]
    error_label: TemplateChild<gtk4::Label>,
}

#[glib::object_subclass]
impl ObjectSubclass for InoxWgpuPreviewImp {
    const NAME: &'static str = "PIInoxWgpuPreview";
    type Type = InoxWgpuPreview;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for InoxWgpuPreviewImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for InoxWgpuPreviewImp {
    fn realize(&self) {
        self.parent_realize();
    }
}

impl BoxImpl for InoxWgpuPreviewImp {}

glib::wrapper! {
    pub struct InoxWgpuPreview(ObjectSubclass<InoxWgpuPreviewImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl InoxWgpuPreview {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        selfish.imp().wgpu_view.bind(document);

        selfish
    }

    pub fn display_error(&self, error: &str) {
        self.append(&*self.imp().error_view);
        self.imp().error_label.set_label(error);
    }
}
