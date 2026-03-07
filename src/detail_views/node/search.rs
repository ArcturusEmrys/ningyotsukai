use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::node::InoxNodeUuid;

use crate::document::Document;
use crate::json::JsonValueExt;
use crate::navigation_item::{NavigationItem, Path};
use crate::string_ext::StrExt;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/node_search.ui")]
pub struct NodeSearchImp {
    state: RefCell<Option<(Arc<Mutex<Document>>, gio::ListStore)>>,

    #[template_child]
    id_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    name_field: TemplateChild<gtk4::Entry>,

    #[template_child]
    results_view: TemplateChild<gtk4::ColumnView>,
    #[template_child]
    results_selection: TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    id_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    name_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    kind_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    zsort_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    link_factory: TemplateChild<gtk4::SignalListItemFactory>,
}

#[glib::object_subclass]
impl ObjectSubclass for NodeSearchImp {
    const NAME: &'static str = "PIPuppetNodeSearch";
    type Type = NodeSearch;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for NodeSearchImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for NodeSearchImp {}

impl BoxImpl for NodeSearchImp {}

glib::wrapper! {
    pub struct NodeSearch(ObjectSubclass<NodeSearchImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl NodeSearch {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        let list_store = gio::ListStore::builder().build();

        *selfish.imp().state.borrow_mut() = Some((document, list_store));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let borrow = self.imp().state.borrow();
        let (document_arc, list_store) = borrow.as_ref().unwrap();
        self.imp().results_selection.set_model(Some(list_store));

        let action_group = gio::SimpleActionGroup::new();
        let search_self = self.clone();
        action_group.add_action_entries([gio::ActionEntry::builder("search")
            .activate(move |_, _, _| search_self.search())
            .build()]);
        self.insert_action_group("form", Some(&action_group));

        self.imp().id_factory.connect_setup(|_fac, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            list_item.set_child(Some(
                &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
            ));
        });

        self.imp().id_factory.connect_bind(move |_fac, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_item = list_item.item().unwrap();
            let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();
            let id: u32 = nav.as_puppet_node().unwrap().into();

            let label_child = list_item.child().unwrap();
            let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

            label.set_text(&format!("{}", id));
        });

        self.imp().name_factory.connect_setup(|_fac, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            list_item.set_child(Some(
                &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
            ));
        });

        let name_document = document_arc.clone();
        self.imp().name_factory.connect_bind(move |_fac, object| {
            let document = name_document.lock().unwrap();

            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_item = list_item.item().unwrap();
            let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();
            let name = nav.name(&document);

            let label_child = list_item.child().unwrap();
            let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

            label.set_text(&name.escape_nulls());
        });

        self.imp().kind_factory.connect_setup(|_fac, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            list_item.set_child(Some(&gtk4::Label::builder().build()));
        });

        let kind_document = document_arc.clone();
        self.imp().kind_factory.connect_bind(move |_fac, object| {
            let document = kind_document.lock().unwrap();

            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_item = list_item.item().unwrap();
            let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();

            //Inox doesn't actually store the Kind of a node, so we have to get it ourselves.
            //This is really dumb.
            let json_path = nav.as_json_path(&document).unwrap();
            let node_json = document.puppet_json.traverse_path(json_path.as_path());

            if let Some(kind) = node_json.and_then(|j| j.as_object()?.get("type")?.as_str()) {
                let label_child = list_item.child().unwrap();
                let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                label.set_text(&kind.escape_nulls());
            }
        });

        self.imp().zsort_factory.connect_setup(|_fac, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            list_item.set_child(Some(&gtk4::Label::builder().build()));
        });

        let zsort_document = document_arc.clone();
        self.imp().zsort_factory.connect_bind(move |_fac, object| {
            let document = zsort_document.lock().unwrap();

            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_item = list_item.item().unwrap();
            let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();
            let label_child = list_item.child().unwrap();
            let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

            label.set_text(&format!(
                "{}",
                document
                    .model
                    .puppet
                    .nodes()
                    .get_node(nav.as_puppet_node().unwrap())
                    .unwrap()
                    .zsort
            ));
        });

        let link_factory_document = document_arc.clone();
        self.imp().link_factory.connect_bind(move |_fac, object| {
            let document = link_factory_document.lock().unwrap();

            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_item = list_item.item().unwrap();
            let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();

            let jump_button = gtk4::Button::builder()
                .label("Jump to node...")
                .action_name("win.jump")
                .action_target(&nav.as_path().to_variant())
                .build();
            let json_jump_button = gtk4::Button::builder()
                .label("Jump to JSON...")
                .action_name("win.jump")
                .action_target(&nav.as_json_path(&document).unwrap().to_variant())
                .build();

            let gtkbox = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .build();
            gtkbox.append(&jump_button);
            gtkbox.append(&json_jump_button);

            list_item.set_child(Some(&gtkbox));
        });
    }

    fn search(&self) {
        let (document_arc, results) = self.imp().state.borrow().as_ref().unwrap().clone();
        let document = document_arc.lock().unwrap();

        // GTK3 had a get_window, GTK4 removed it.
        // Dunno why, but there's a forum thread where someone
        // basically says you shouldn't need to get the current
        // window because you don't need to touch GDK as often.
        // Guess he didn't read GTK's own alert API.
        let mut maybe_window = self.parent();
        let mut window = None;
        while maybe_window.is_some() {
            if let Some(awindow) = maybe_window
                .as_ref()
                .unwrap()
                .downcast_ref::<gtk4::Window>()
            {
                window = Some(awindow.clone());
                break;
            }

            maybe_window = maybe_window.unwrap().parent();
        }

        let uuid = self.imp().id_field.buffer().text();
        let uuid = match str::parse::<u32>(&uuid) {
            Ok(val) => Some(val),
            Err(e) if uuid != "" => {
                return gtk4::AlertDialog::builder()
                    .message("Invalid UUID")
                    .detail(format!("{} is not a valid integer", uuid))
                    .buttons(["OK"])
                    .modal(true)
                    .build()
                    .show(window.as_ref());
            }
            Err(_) => None,
        };

        let name = self.imp().name_field.buffer().text();
        let mut new_results = vec![];

        for node in document.model.puppet.nodes().iter() {
            if let Some(uuid) = uuid
                && <InoxNodeUuid as Into<u32>>::into(node.uuid) != uuid
            {
                continue;
            }

            if !node.name.contains(&*name) {
                continue;
            }

            new_results.push(NavigationItem::from_node(node.uuid));
        }

        drop(document);

        results.remove_all();
        results.extend_from_slice(&new_results.as_slice());
    }
}
