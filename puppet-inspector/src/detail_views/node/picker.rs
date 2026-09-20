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
use crate::navigation::NavigationItem;
use ningyo_extensions::prelude::*;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/node/picker.ui")]
pub struct NodePickerImp {
    state: RefCell<Option<(Arc<Mutex<Document>>, gio::ListStore)>>,

    #[template_child]
    list: TemplateChild<gtk4::ColumnView>,
    #[template_child]
    selection: TemplateChild<gtk4::SingleSelection>,
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
impl ObjectSubclass for NodePickerImp {
    const NAME: &'static str = "PIPuppetNodePicker";
    type Type = NodePicker;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for NodePickerImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for NodePickerImp {}

impl BoxImpl for NodePickerImp {}

glib::wrapper! {
    pub struct NodePicker(ObjectSubclass<NodePickerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl NodePicker {
    pub fn bind(&self, document: Arc<Mutex<Document>>) {
        let list_store = gio::ListStore::builder().build();

        *self.imp().state.borrow_mut() = Some((document, list_store));

        let borrow = self.imp().state.borrow();
        let (document_arc, list_store) = borrow.as_ref().unwrap();
        self.imp().selection.set_model(Some(list_store));

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
                .action_name("doc.jump")
                .action_target(&nav.as_path().to_variant())
                .build();
            let json_jump_button = gtk4::Button::builder()
                .label("Jump to JSON...")
                .action_name("doc.jump")
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

    /// Set the list of visible nodes in the picker.
    pub fn set_nodes(&self, nodes: &[InoxNodeUuid]) {
        //TODO: Check what nodes are currently selected and try to retain them
        //in the selection set.

        let list_store = self.imp().state.borrow().as_ref().unwrap().1.clone();

        let mut mapped = vec![];
        for node in nodes {
            mapped.push(NavigationItem::from_node(*node));
        }

        list_store.remove_all();
        list_store.extend_from_slice(&mapped);
    }
}
