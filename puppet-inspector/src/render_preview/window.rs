use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;
use ningyo_extensions::WidgetExt2;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::render_preview::opengl::InoxGLPreview;
use crate::render_preview::param_list::RenderParamList;
use crate::render_preview::wgpu::InoxWgpuPreview;
use crate::render_preview::wgpu_inner::MyWgpuArea;

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
    preview_view: TemplateChild<gtk4::Box>,
    #[template_child]
    param_list: TemplateChild<RenderParamList>,
    #[template_child]
    renderer_menu_button: TemplateChild<gtk4::MenuButton>,
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
        self.obj().add_css_class("PuppetInspectorWindow");
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
        MyWgpuArea::ensure_type();

        let selfish: Self = glib::Object::builder().build();

        selfish.imp().param_list.bind(document.clone());

        *selfish.imp().state.borrow_mut() = Some(State {
            document: document.clone(),
        });

        selfish.use_wgpu();

        let render_actions = gio::SimpleActionGroup::new();
        render_actions.add_action_entries([
            gio::ActionEntry::builder("render-with-opengl")
                .activate({
                    let opengl_self = selfish.clone();
                    move |_, _, _| {
                        opengl_self.use_opengl();
                    }
                })
                .build(),
            gio::ActionEntry::builder("render-with-wgpu")
                .activate({
                    let wgpu_self = selfish.clone();
                    move |_, _, _| {
                        wgpu_self.use_wgpu();
                    }
                })
                .build(),
        ]);

        selfish.insert_action_group("render", Some(&render_actions));

        selfish
    }

    pub fn do_param_set(&self, document: &mut Document) {
        self.imp().param_list.do_param_set(document);
    }

    fn use_opengl(&self) {
        let document = self.imp().state.borrow().as_ref().unwrap().document.clone();

        self.imp().preview_view.clear_children();

        self.imp()
            .preview_view
            .append(&InoxGLPreview::new(document));

        self.imp().renderer_menu_button.set_label("OpenGL");
    }

    fn use_wgpu(&self) {
        let document = self.imp().state.borrow().as_ref().unwrap().document.clone();

        self.imp().preview_view.clear_children();

        self.imp()
            .preview_view
            .append(&InoxWgpuPreview::new(document));

        self.imp().renderer_menu_button.set_label("WGPU");
    }
}
