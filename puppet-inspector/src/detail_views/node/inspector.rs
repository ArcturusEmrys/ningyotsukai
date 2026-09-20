use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::node::InoxNodeUuid;
use inox2d::node::components::Drawable;

use crate::detail_views::node::pages::DrawableDetailPage;
use crate::document::Document;
use ningyo_extensions::prelude::*;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/node/inspector.ui")]
pub struct NodeInspectorImp {
    document: RefCell<Option<(Arc<Mutex<Document>>, InoxNodeUuid)>>,

    #[template_child]
    name_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    uuid_label: TemplateChild<gtk4::Entry>,
    #[template_child]
    enabled_field: TemplateChild<gtk4::CheckButton>,
    #[template_child]
    zsort_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_translate_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_translate_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_translate_z_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_rotate_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_rotate_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_rotate_z_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_scale_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    transform_scale_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    lock_to_root_field: TemplateChild<gtk4::CheckButton>,
    #[template_child]
    transform_pixel_snap_field: TemplateChild<gtk4::CheckButton>,
    #[template_child]
    detail_pages: TemplateChild<gtk4::Stack>,
}

#[glib::object_subclass]
impl ObjectSubclass for NodeInspectorImp {
    const NAME: &'static str = "PIPuppetNodeInspector";
    type Type = NodeInspector;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for NodeInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for NodeInspectorImp {}

impl GridImpl for NodeInspectorImp {}

glib::wrapper! {
    pub struct NodeInspector(ObjectSubclass<NodeInspectorImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl NodeInspector {
    pub fn new(document: Arc<Mutex<Document>>, uuid: InoxNodeUuid) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((document, uuid));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let (document_arc, uuid) = self.imp().document.borrow().as_ref().unwrap().clone();
        let document = document_arc.lock().unwrap();
        let node = document
            .puppet_data()
            .nodes()
            .get_node(uuid)
            .expect("valid node");

        self.imp()
            .name_field
            .buffer()
            .set_text(node.name.escape_nulls().as_ref());
        self.imp()
            .uuid_label
            .buffer()
            .set_text(&format!("{}", Into::<u32>::into(uuid)));
        self.imp().enabled_field.set_active(node.enabled);
        self.imp()
            .zsort_field
            .buffer()
            .set_text(&format!("{}", node.zsort));
        self.imp().lock_to_root_field.set_active(node.lock_to_root);

        self.imp()
            .transform_translate_x_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.translation.x));
        self.imp()
            .transform_translate_y_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.translation.y));
        self.imp()
            .transform_translate_z_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.translation.z));
        self.imp()
            .transform_rotate_x_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.rotation.x));
        self.imp()
            .transform_rotate_y_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.rotation.y));
        self.imp()
            .transform_rotate_z_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.rotation.z));
        self.imp()
            .transform_scale_x_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.scale.x));
        self.imp()
            .transform_scale_y_field
            .buffer()
            .set_text(&format!("{}", node.trans_offset.scale.y));
        self.imp()
            .transform_pixel_snap_field
            .set_active(node.trans_offset.pixel_snap);

        let is_drawable = document
            .model
            .puppet
            .world()
            .get::<Drawable>(uuid)
            .is_some();

        drop(document);

        if is_drawable {
            self.imp()
                .detail_pages
                .add_named(
                    &DrawableDetailPage::new(document_arc.clone(), uuid),
                    Some("Drawable"),
                )
                .set_title("Drawable");
        }
    }
}
