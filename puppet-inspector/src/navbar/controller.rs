use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use ningyo_extensions::StrExt;
use ningyo_extensions::WidgetExt2;

use std::cell::RefCell;

use std::sync::{Arc, Mutex};

use crate::document::{Document, DocumentController};
use crate::navigation::Path;

#[derive(Default)]
pub struct NavbarControllerState {
    open_doc: Option<(Arc<Mutex<Document>>, DocumentController)>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/navbar/controller.ui")]
pub struct NavbarControllerImp {
    state: RefCell<NavbarControllerState>,

    #[template_child]
    up_button: TemplateChild<gtk4::Button>,

    #[template_child]
    path_components: TemplateChild<gtk4::Box>,
}

#[glib::object_subclass]
impl ObjectSubclass for NavbarControllerImp {
    const NAME: &'static str = "PINavbarController";
    type Type = NavbarController;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for NavbarControllerImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for NavbarControllerImp {}

impl BoxImpl for NavbarControllerImp {}

glib::wrapper! {
    pub struct NavbarController(ObjectSubclass<NavbarControllerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl NavbarController {
    pub fn set_document_and_parent(
        &self,
        document: Arc<Mutex<Document>>,
        parent: DocumentController,
    ) {
        self.imp().state.borrow_mut().open_doc = Some((document, parent));

        self.imp().up_button.connect_clicked({
            let active_self = self.downgrade();
            move |_| {
                if let Some(active_self) = active_self.upgrade() {
                    active_self
                        .imp()
                        .state
                        .borrow()
                        .open_doc
                        .as_ref()
                        .unwrap()
                        .1
                        .jump_up();
                }
            }
        });
    }

    fn prepend_component(&self, path: &Path, document: &Document) {
        let component = gtk4::Button::builder()
            .label(path.name(document).escape_nulls())
            .build();

        component.connect_clicked({
            let active_self = self.downgrade();
            let target_path = path.clone();
            move |_| {
                if let Some(active_self) = active_self.upgrade() {
                    active_self
                        .imp()
                        .state
                        .borrow()
                        .open_doc
                        .as_ref()
                        .unwrap()
                        .1
                        .jump_to(target_path.clone());
                }
            }
        });

        self.imp().path_components.prepend(&component);
    }

    /// Change the currently displayed path.
    pub fn set_path(&self, mut path: Path) {
        self.imp().path_components.clear_children();

        let document_ref = self
            .imp()
            .state
            .borrow()
            .open_doc
            .as_ref()
            .unwrap()
            .0
            .clone();
        let document = document_ref.lock().unwrap();

        self.prepend_component(&path, &document);

        while let Some(parent) = path.parent(&*document) {
            self.prepend_component(&parent, &document);

            path = parent;
        }
    }
}
