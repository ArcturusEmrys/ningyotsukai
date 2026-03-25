use gio;
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::subclass::prelude::*;

use crate::document::DocumentController;
use crate::tracker::TrackerManager;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/window/controller.ui")]
pub struct WindowControllerImp {
    #[template_child]
    main_menu: TemplateChild<gio::MenuModel>,
    #[template_child]
    main_menu_button: TemplateChild<gtk4::MenuButton>,
    #[template_child]
    document_controller: TemplateChild<DocumentController>,

    // external / non-GTK goes here
    tracker_manager: RefCell<Option<Rc<TrackerManager>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for WindowControllerImp {
    const NAME: &'static str = "NGTWindowController";
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

impl WidgetImpl for WindowControllerImp {
    fn unrealize(&self) {
        //We need to drop our tracker manager, otherwise we keep the application open
        self.tracker_manager.borrow_mut().take();
        self.parent_unrealize();
    }
}

impl WindowImpl for WindowControllerImp {}

impl ApplicationWindowImpl for WindowControllerImp {}

glib::wrapper! {
    pub struct WindowController(ObjectSubclass<WindowControllerImp>)
        @extends gtk4::ApplicationWindow, gtk4::Window, gtk4::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl WindowController {
    pub fn new(app: &gtk4::Application, tracker_manager: Rc<TrackerManager>) -> Self {
        let selfish: WindowController =
            glib::Object::builder().property("application", app).build();

        *selfish.imp().tracker_manager.borrow_mut() = Some(tracker_manager);

        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let main_menu = self.imp().main_menu.clone();
        let main_menu_button = self.imp().main_menu_button.clone();
        main_menu_button.set_menu_model(Some(&main_menu));
    }
}
