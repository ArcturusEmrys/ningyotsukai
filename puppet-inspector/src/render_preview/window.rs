use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::render_preview::opengl::InoxGLPreview;
use crate::render_preview::param_list::RenderParamList;

struct State {
    document: Arc<Mutex<Document>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/render_preview/window.ui")]
pub struct InoxRenderPreviewImp {
    state: RefCell<Option<State>>,

    #[template_child]
    paned_view: TemplateChild<gtk4::Paned>,
    #[template_child]
    preview_view: TemplateChild<InoxGLPreview>,
    #[template_child]
    param_list: TemplateChild<RenderParamList>,
}

#[glib::object_subclass]
impl ObjectSubclass for InoxRenderPreviewImp {
    const NAME: &'static str = "PIInoxRenderPreview";
    type Type = InoxRenderPreview;
    type ParentType = gtk4::Window;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for InoxRenderPreviewImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for InoxRenderPreviewImp {}

impl WindowImpl for InoxRenderPreviewImp {}

glib::wrapper! {
    pub struct InoxRenderPreview(ObjectSubclass<InoxRenderPreviewImp>)
        @extends gtk4::Window, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible,
            gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl InoxRenderPreview {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        selfish.imp().param_list.bind(document.clone());

        *selfish.imp().state.borrow_mut() = Some(State {
            document: document.clone(),
        });

        selfish.imp().preview_view.bind(document);

        selfish
    }

    pub fn do_param_set(&self, document: &mut Document) {
        self.imp().param_list.do_param_set(document);
    }
}
