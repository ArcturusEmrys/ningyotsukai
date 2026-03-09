use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::params::ParamUuid;

use crate::document::Document;
use crate::string_ext::StrExt;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/param_inspector.ui")]
pub struct ParamInspectorImp {
    document: RefCell<Option<(Arc<Mutex<Document>>, ParamUuid)>>,

    #[template_child]
    name_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    uuid_label: TemplateChild<gtk4::Label>,
    #[template_child]
    is_vec2_check: TemplateChild<gtk4::CheckButton>,
}

#[glib::object_subclass]
impl ObjectSubclass for ParamInspectorImp {
    const NAME: &'static str = "PIPuppetParamInspector";
    type Type = ParamInspector;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ParamInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for ParamInspectorImp {}

impl GridImpl for ParamInspectorImp {}

glib::wrapper! {
    pub struct ParamInspector(ObjectSubclass<ParamInspectorImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl ParamInspector {
    pub fn new(document: Arc<Mutex<Document>>, param_uuid: ParamUuid) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((document, param_uuid));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let (document_arc, param_uuid) = self.imp().document.borrow().as_ref().unwrap().clone();
        let document = document_arc.lock().unwrap();
        let (name, param) = document
            .puppet_data()
            .params()
            .iter()
            .find(|(_k, v)| v.uuid == param_uuid)
            .expect("valid param");

        self.imp()
            .name_field
            .buffer()
            .set_text(name.escape_nulls().as_ref());
        self.imp()
            .uuid_label
            .set_label(&format!("{}", param.uuid.0));
        self.imp().is_vec2_check.set_active(param.is_vec2);
    }
}
