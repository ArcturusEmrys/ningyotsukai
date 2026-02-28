use glib;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use inox2d;
use std::cell::RefCell;

use crate::document::Document;

pub enum Section {
    PuppetMeta,
    PuppetPhysics,
    PuppetNode,
    PuppetParams,
    ModelTextures,
    VendorData,
}

pub enum PathComponent {
    Section(Section),
    PuppetNode(inox2d::node::InoxNodeUuid),
}

#[derive(Default)]
pub struct NavigationItemImp {
    pub path: RefCell<Option<PathComponent>>,
}

#[glib::object_subclass]
impl ObjectSubclass for NavigationItemImp {
    const NAME: &'static str = "PINavigationItem";
    type Type = NavigationItem;
}

impl ObjectImpl for NavigationItemImp {}

glib::wrapper! {
    pub struct NavigationItem(ObjectSubclass<NavigationItemImp>);
}

impl NavigationItem {
    pub fn new(path: PathComponent) -> Self {
        let selfpoi: Self = glib::Object::builder().build();

        *(selfpoi.imp().path.borrow_mut()) = Some(path);

        selfpoi
    }

    pub fn name<'a>(&self, document: &'a Document) -> &'a str {
        match self.imp().path.borrow().as_ref().expect("a path") {
            PathComponent::Section(Section::ModelTextures) => "Textures",
            PathComponent::Section(Section::PuppetMeta) => "Metadata",
            PathComponent::Section(Section::PuppetNode) => "Nodes",
            PathComponent::Section(Section::PuppetParams) => "Params",
            PathComponent::Section(Section::PuppetPhysics) => "Physics",
            PathComponent::Section(Section::VendorData) => "VendorData",
            PathComponent::PuppetNode(node_id) => {
                let node = document.puppet_data.nodes().get_node(*node_id);

                if let Some(node) = node {
                    &node.name
                } else {
                    "<MISSING OR INVALID NODE>"
                }
            }
        }
    }

    pub fn child_list(&self, document: &Document) -> Option<gio::ListModel> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            PathComponent::Section(Section::PuppetNode) => {
                let root_node = document.puppet_data.nodes().root_node_id;
                let list = gio::ListStore::builder().build();
                list.extend_from_slice(&[Self::new(PathComponent::PuppetNode(root_node))]);

                Some(list.into())
            }
            PathComponent::PuppetNode(node_id) => {
                let mut child_node_paths = vec![];
                for child_node in document.puppet_data.nodes().get_children(*node_id) {
                    child_node_paths.push(Self::new(PathComponent::PuppetNode(child_node.uuid)));
                }

                if child_node_paths.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(child_node_paths.as_slice());
                Some(list.into())
            }
            _ => None,
        }
    }
}
