use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::node::InoxNodeUuid;

use crate::detail_views::node::picker::NodePicker;
use crate::document::Document;
use crate::navigation::NavigationItem;
use ningyo_extensions::prelude::*;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/node/search.ui")]
pub struct NodeSearchImp {
    state: RefCell<Option<Arc<Mutex<Document>>>>,

    #[template_child]
    id_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    name_field: TemplateChild<gtk4::Entry>,

    #[template_child]
    results_view: TemplateChild<NodePicker>,
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

        *selfish.imp().state.borrow_mut() = Some(document);
        selfish.bind();
        selfish.search();

        selfish
    }

    fn bind(&self) {
        let borrow = self.imp().state.borrow();
        self.imp().results_view.bind(borrow.clone().unwrap());

        let action_group = gio::SimpleActionGroup::new();
        let search_self = self.clone();
        action_group.add_action_entries([gio::ActionEntry::builder("search")
            .activate(move |_, _, _| search_self.search())
            .build()]);
        self.insert_action_group("form", Some(&action_group));
    }

    fn search(&self) {
        let document_arc = self.imp().state.borrow().as_ref().unwrap().clone();
        let document = document_arc.lock().unwrap();

        let window = self.window();

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

            new_results.push(node.uuid);
        }

        drop(document);

        self.imp().results_view.set_nodes(&new_results);
    }
}
