use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::Arc;

use crate::document::Document;
use crate::string_ext::StrExt;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/physics_inspector.ui")]
pub struct PhysicsInspectorImp {
    document: RefCell<Option<Arc<Document>>>,

    #[template_child]
    ppm_scale_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    gconstant_field: TemplateChild<gtk4::TextView>,
}

#[glib::object_subclass]
impl ObjectSubclass for PhysicsInspectorImp {
    const NAME: &'static str = "PIPuppetPhysicsInspector";
    type Type = PhysicsInspector;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for PhysicsInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for PhysicsInspectorImp {}

impl GridImpl for PhysicsInspectorImp {}

glib::wrapper! {
    pub struct PhysicsInspector(ObjectSubclass<PhysicsInspectorImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl PhysicsInspector {
    pub fn new(document: Arc<Document>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some(document);
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let document = self.imp().document.borrow().as_ref().unwrap().clone();

        self.imp().ppm_scale_field.buffer().set_text(&format!(
            "{}",
            document.puppet_data.physics().pixels_per_meter
        ));
    }
}
