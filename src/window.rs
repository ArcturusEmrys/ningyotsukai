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
use std::io::Read;

use crate::document::Document;
use crate::navigation_item::{NavigationItem, PathComponent, Section};
use crate::string_ext::StrExt;

/// For some reason, glib-rs does not support mutating private/impl structs.
/// Hence the mutability hack.
#[derive(Default)]
pub struct WindowControllerState {
    open_doc: Option<Document>,
    navigation_tree: Option<gtk4::TreeListModel>,
    root_list: Option<gio::ListStore>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/window.ui")]
pub struct WindowControllerImp {
    #[template_child]
    filepicker: TemplateChild<gtk4::FileDialog>,
    #[template_child]
    navigation_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    navigation_selection: TemplateChild<gtk4::SingleSelection>,
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

        self.imp().state.borrow_mut().open_doc = Some(Document::open(stream_adapter)?);

        self.populate_navigation();

        Ok(())
    }

    pub fn populate_navigation(&self) {
        let mut state = self.imp().state.borrow_mut();
        if state.root_list.is_none() {
            state.root_list = Some(gio::ListStore::builder().build());
        }

        let root_list = state.root_list.clone().unwrap();

        if state.navigation_tree.is_none() {
            let callback_self = self.clone();
            let navigation_tree =
                gtk4::TreeListModel::new(root_list.clone(), false, false, move |node| {
                    let nav = node
                        .clone()
                        .downcast::<NavigationItem>()
                        .expect("our own child");
                    let state = callback_self.imp().state.borrow();
                    let document = state.open_doc.as_ref();

                    if let Some(document) = document {
                        nav.child_list(document)
                    } else {
                        None
                    }
                });
            state.navigation_tree = Some(navigation_tree.clone());

            self.imp()
                .navigation_selection
                .set_model(Some(&navigation_tree));
        }

        drop(state);

        let factory = self.imp().navigation_factory.clone();
        let factory_callback_self = self.clone();

        factory.connect_setup(|_factory, list_item| {
            let label = gtk4::Label::new(None);
            let tree_expander = gtk4::TreeExpander::builder().build();

            tree_expander.set_child(Some(&label));

            let list_item = list_item
                .downcast_ref::<gtk4::ListItem>()
                .expect("list item");

            list_item.set_child(Some(&tree_expander));
            list_item.set_property("focusable", false);
        });

        factory.connect_bind(move |_factory, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk4::ListItem>()
                .expect("list item");

            let mut maybe_nav = list_item.item().expect("list items to have a child");
            let mut tree_list_row = None;
            while maybe_nav.clone().downcast::<NavigationItem>().is_err() {
                let tlr = maybe_nav
                    .downcast::<gtk4::TreeListRow>()
                    .expect("valid child list item");
                tree_list_row = Some(tlr.clone());
                if let Some(child) = tlr.item() {
                    maybe_nav = child;
                } else {
                    panic!("No navigation child!");
                }
            }

            let nav = maybe_nav
                .downcast::<NavigationItem>()
                .expect("our own child");
            let tree_item = list_item
                .child()
                .and_downcast::<gtk4::TreeExpander>()
                .expect("our own tree expander");

            tree_item.set_list_row(tree_list_row.as_ref());

            let label = tree_item
                .child()
                .and_downcast::<gtk4::Label>()
                .expect("our own label");
            let state = factory_callback_self.imp().state.borrow();

            if let Some(document) = state.open_doc.as_ref() {
                label.set_label(nav.name(&document).trim_nulls());
            } else {
                label.set_label("Wot! No document?");
            }
        });

        root_list.extend_from_slice(&[
            NavigationItem::new(PathComponent::Section(Section::PuppetMeta)),
            NavigationItem::new(PathComponent::Section(Section::PuppetPhysics)),
            NavigationItem::new(PathComponent::Section(Section::PuppetNode)),
            NavigationItem::new(PathComponent::Section(Section::PuppetParams)),
            NavigationItem::new(PathComponent::Section(Section::ModelTextures)),
            NavigationItem::new(PathComponent::Section(Section::VendorData)),
        ]);
    }
}
