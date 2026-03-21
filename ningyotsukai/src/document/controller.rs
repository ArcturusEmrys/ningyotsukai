use gio;
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::document::model::Document;
use crate::stage::StageWidget;

#[derive(Default)]
pub struct DocumentControllerState {
    document: Arc<Mutex<Document>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/document/controller.ui")]
pub struct DocumentControllerImp {
    #[template_child]
    stage: TemplateChild<StageWidget>,
    state: RefCell<DocumentControllerState>,
}

#[glib::object_subclass]
impl ObjectSubclass for DocumentControllerImp {
    const NAME: &'static str = "NGTDocumentController";
    type Type = DocumentController;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for DocumentControllerImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for DocumentControllerImp {}

impl BoxImpl for DocumentControllerImp {}

glib::wrapper! {
    pub struct DocumentController(ObjectSubclass<DocumentControllerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl DocumentController {
    pub fn new(app: &gtk4::Application) -> Self {
        let selfish: DocumentController =
            glib::Object::builder().property("application", app).build();

        selfish.bind();

        selfish
    }

    fn bind(&self) {
        self.imp()
            .stage
            .set_document(self.imp().state.borrow().document.clone());
    }
}
