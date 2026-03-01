use glib;
use gtk4::subclass::prelude::*;

use inox2d;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt::{Debug, Error as FmtError, Formatter};
use std::sync::Arc;

use crate::detail_views::{MetadataInspector, PhysicsInspector};
use crate::document::Document;

#[derive(Debug)]
pub enum Section {
    PuppetMeta,
    PuppetPhysics,
    PuppetNode,
    PuppetParams,
    ModelTextures,
    VendorData,
}

#[derive(Debug)]
pub enum PathComponent {
    Section(Section),
    PuppetNode(inox2d::node::InoxNodeUuid),
    PuppetParam(String),
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

    pub fn name<'a>(&self, document: &'a Document) -> Cow<'a, str> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            PathComponent::Section(Section::ModelTextures) => "Textures".into(),
            PathComponent::Section(Section::PuppetMeta) => "Metadata".into(),
            PathComponent::Section(Section::PuppetNode) => "Nodes".into(),
            PathComponent::Section(Section::PuppetParams) => "Params".into(),
            PathComponent::Section(Section::PuppetPhysics) => "Physics".into(),
            PathComponent::Section(Section::VendorData) => "VendorData".into(),
            PathComponent::PuppetNode(node_id) => {
                let node = document.puppet_data.nodes().get_node(*node_id);

                if let Some(node) = node {
                    (&node.name).into()
                } else {
                    "<MISSING OR INVALID NODE>".into()
                }
            }
            PathComponent::PuppetParam(name) => name.to_string().into(),
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
            PathComponent::Section(Section::PuppetParams) => {
                let mut param_paths = vec![];
                for param in document.puppet_data.params.keys() {
                    param_paths.push(Self::new(PathComponent::PuppetParam(param.clone())));
                }

                if param_paths.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(param_paths.as_slice());
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

    pub fn child_inspector(&self, document: Arc<Document>) -> gtk4::Widget {
        match self.imp().path.borrow().as_ref().expect("a path") {
            PathComponent::Section(Section::PuppetMeta) => MetadataInspector::new(document).into(),
            PathComponent::Section(Section::PuppetPhysics) => {
                PhysicsInspector::new(document).into()
            }
            path => gtk4::Label::builder()
                .label(format!("Not yet implemented: {:?}", path))
                .build()
                .into(),
        }
    }
}
