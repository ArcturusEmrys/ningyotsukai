use gio;
use glib;
use gtk4;

use gio::prelude::*;
use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::error::Error;

use std::sync::{Arc, Mutex};

use crate::document::{Document, DocumentController};

/// For some reason, glib-rs does not support mutating private/impl structs.
/// Hence the mutability hack.
#[derive(Default)]
pub struct WindowControllerState {
    open_doc: Option<Arc<Mutex<Document>>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/window.ui")]
pub struct WindowControllerImp {
    #[template_child]
    filepicker: TemplateChild<gtk4::FileDialog>,
    #[template_child]
    main_menu: TemplateChild<gio::MenuModel>,
    #[template_child]
    main_menu_button: TemplateChild<gtk4::MenuButton>,
    #[template_child]
    contents: TemplateChild<gtk4::Box>,
    state: RefCell<WindowControllerState>,
}

#[glib::object_subclass]
impl ObjectSubclass for WindowControllerImp {
    const NAME: &'static str = "PIWindowController";
    type Type = WindowController;
    type ParentType = gtk4::ApplicationWindow;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for WindowControllerImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for WindowControllerImp {}

impl WindowImpl for WindowControllerImp {}

impl ApplicationWindowImpl for WindowControllerImp {}

glib::wrapper! {
    pub struct WindowController(ObjectSubclass<WindowControllerImp>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl WindowController {
    pub fn new(app: &gtk4::Application) -> Self {
        let selfish: WindowController =
            glib::Object::builder().property("application", app).build();

        let main_menu = selfish.imp().main_menu.clone();
        let main_menu_button = selfish.imp().main_menu_button.clone();
        main_menu_button.set_menu_model(Some(&main_menu));

        let picker = selfish.imp().filepicker.clone();
        let callback_self = selfish.clone();
        selfish.add_action_entries([gio::ActionEntry::builder("open")
            .activate(move |window: &WindowController, _, _| {
                let callback_self = callback_self.clone();

                picker.open(
                    Some(window),
                    Some(&gio::Cancellable::new()),
                    move |file_or_error| {
                        let maybe_error: Result<(), Box<dyn Error>> = (|| {
                            callback_self.open_document(file_or_error?)?;
                            Ok(())
                        })();

                        if let Err(e) = maybe_error {
                            eprintln!("{:?}", e);
                        }
                    },
                );
            })
            .build()]);

        selfish
    }

    pub fn open_document(&self, file: gio::File) -> Result<(), Box<dyn Error>> {
        // TODO: Create some actual UI surface from all of this.
        // TODO: async loading

        let stream = file.read(Some(&gio::Cancellable::new()))?;
        let stream_adapter = crate::io_adapter::FileIn::from(stream);

        let document = Arc::new(Mutex::new(Document::open(stream_adapter)?));

        self.imp().state.borrow_mut().open_doc = Some(document.clone());

        let contents = self.imp().contents.clone();
        while contents.first_child().is_some() {
            contents.remove(&contents.first_child().unwrap());
        }

        contents.append(&DocumentController::new(document));

        Ok(())
    }
}
