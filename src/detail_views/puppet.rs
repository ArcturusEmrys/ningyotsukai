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
#[template(resource = "/live/arcturus/puppet-inspector/metadata_inspector.ui")]
pub struct MetadataInspectorImp {
    document: RefCell<Option<Arc<Document>>>,

    #[template_child]
    name_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    version_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    rigger_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    artist_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    copyright_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    license_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    contact_field: TemplateChild<gtk4::TextView>,
    #[template_child]
    reference_field: TemplateChild<gtk4::TextView>,
}

#[glib::object_subclass]
impl ObjectSubclass for MetadataInspectorImp {
    const NAME: &'static str = "PIPuppetMetadataInspector";
    type Type = MetadataInspector;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for MetadataInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for MetadataInspectorImp {}

impl GridImpl for MetadataInspectorImp {}

glib::wrapper! {
    pub struct MetadataInspector(ObjectSubclass<MetadataInspectorImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl MetadataInspector {
    pub fn new(document: Arc<Document>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some(document);
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let document = self.imp().document.borrow().as_ref().unwrap().clone();

        self.imp().name_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .name
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp()
            .version_field
            .buffer()
            .set_text(document.puppet_data.meta.version.escape_nulls().as_ref());
        self.imp().rigger_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .rigger
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp().artist_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .artist
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp().copyright_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .copyright
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp().license_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .license_url
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp().contact_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .contact
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
        self.imp().reference_field.buffer().set_text(
            document
                .puppet_data
                .meta
                .reference
                .as_deref()
                .unwrap_or("")
                .escape_nulls()
                .as_ref(),
        );
    }
}
