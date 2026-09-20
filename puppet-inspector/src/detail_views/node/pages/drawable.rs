use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;
use inox2d::node::components::{BlendMode, Drawable};

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::node::InoxNodeUuid;

use crate::document::Document;
use ningyo_extensions::prelude::*;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/node/pages/drawable.ui")]
pub struct DrawableDetailPageImp {
    document: RefCell<Option<(Arc<Mutex<Document>>, InoxNodeUuid)>>,

    #[template_child]
    blend_mode: TemplateChild<gtk4::Entry>,
    #[template_child]
    tint_r: TemplateChild<gtk4::Entry>,
    #[template_child]
    tint_g: TemplateChild<gtk4::Entry>,
    #[template_child]
    tint_b: TemplateChild<gtk4::Entry>,
    #[template_child]
    screen_tint_r: TemplateChild<gtk4::Entry>,
    #[template_child]
    screen_tint_g: TemplateChild<gtk4::Entry>,
    #[template_child]
    screen_tint_b: TemplateChild<gtk4::Entry>,
    #[template_child]
    opacity: TemplateChild<gtk4::Entry>,
    #[template_child]
    mask_threshold: TemplateChild<gtk4::Entry>,
}

#[glib::object_subclass]
impl ObjectSubclass for DrawableDetailPageImp {
    const NAME: &'static str = "PIDrawableDetailPage";
    type Type = DrawableDetailPage;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for DrawableDetailPageImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for DrawableDetailPageImp {}

impl GridImpl for DrawableDetailPageImp {}

glib::wrapper! {
    pub struct DrawableDetailPage(ObjectSubclass<DrawableDetailPageImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl DrawableDetailPage {
    pub fn new(document: Arc<Mutex<Document>>, uuid: InoxNodeUuid) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((document, uuid));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let document = self.imp().document.borrow();
        let (document_arc, uuid) = document.as_ref().unwrap();
        let document = document_arc.lock().unwrap();

        let Some(drawable) = document.model.puppet.world().get::<Drawable>(*uuid) else {
            //TODO: Show an error.
            return;
        };

        self.imp()
            .blend_mode
            .buffer()
            .set_text(match drawable.blending.mode {
                BlendMode::Normal => "Normal",
                BlendMode::Screen => "Screen",
                BlendMode::ClipToLower => "Clip To Lower",
                BlendMode::ColorDodge => "Color Dodge",
                BlendMode::LinearDodge => "LinearDodge",
                BlendMode::Multiply => "Multiply",
                BlendMode::SliceFromLower => "Slice From Lower",
            });

        self.imp()
            .tint_r
            .buffer()
            .set_text(&format!("{}", drawable.blending.tint.x));
        self.imp()
            .tint_g
            .buffer()
            .set_text(&format!("{}", drawable.blending.tint.y));
        self.imp()
            .tint_b
            .buffer()
            .set_text(&format!("{}", drawable.blending.tint.z));

        self.imp()
            .screen_tint_r
            .buffer()
            .set_text(&format!("{}", drawable.blending.screen_tint.x));
        self.imp()
            .screen_tint_g
            .buffer()
            .set_text(&format!("{}", drawable.blending.screen_tint.y));
        self.imp()
            .screen_tint_b
            .buffer()
            .set_text(&format!("{}", drawable.blending.screen_tint.z));

        self.imp()
            .opacity
            .buffer()
            .set_text(&format!("{}", drawable.blending.opacity));

        self.imp()
            .mask_threshold
            .buffer()
            .set_text(if let Some(masks) = &drawable.masks {
                format!("{}", masks.threshold)
            } else {
                "N/A".to_string()
            });
    }
}
