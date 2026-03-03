use glib;
use gtk4::subclass::prelude::*;

use inox2d;
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use json::JsonValue;

use crate::detail_views::{
    InoxRenderPreview, JsonInspector, JsonValueExt, MetadataInspector, NodeInspector,
    ParamInspector, PhysicsInspector,
};
use crate::document::Document;

#[derive(Debug, Clone)]
pub enum JsonNavigationPath {
    ObjectKey(String),
    ListIndex(usize),
}

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
pub enum Path {
    Section(Section),
    PuppetNode(inox2d::node::InoxNodeUuid),
    PuppetParam(String),
    PuppetJson(Vec<JsonNavigationPath>),
    VendorJson(usize, Vec<JsonNavigationPath>),
    RenderPreview,
}

#[derive(Default)]
pub struct NavigationItemImp {
    pub path: RefCell<Option<Path>>,
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
    pub fn new(path: Path) -> Self {
        let selfpoi: Self = glib::Object::builder().build();

        *(selfpoi.imp().path.borrow_mut()) = Some(path);

        selfpoi
    }

    pub fn name<'a>(&self, document: &'a Document) -> Cow<'a, str> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::Section(Section::ModelTextures) => "Textures".into(),
            Path::Section(Section::PuppetMeta) => "Metadata".into(),
            Path::Section(Section::PuppetNode) => "Nodes".into(),
            Path::Section(Section::PuppetParams) => "Params".into(),
            Path::Section(Section::PuppetPhysics) => "Physics".into(),
            Path::Section(Section::VendorData) => "VendorData".into(),
            Path::PuppetNode(node_id) => {
                let node = document.puppet_data().nodes().get_node(*node_id);

                if let Some(node) = node {
                    (&node.name).into()
                } else {
                    "<MISSING OR INVALID NODE>".into()
                }
            }
            Path::PuppetParam(name) => name.to_string().into(),
            Path::PuppetJson(path) => {
                if let Some(first) = path.last() {
                    match first {
                        JsonNavigationPath::ObjectKey(key) => format!("{}", key).into(),
                        JsonNavigationPath::ListIndex(index) => format!("{}", index).into(),
                    }
                } else {
                    format!("Puppet JSON").into()
                }
            }
            Path::VendorJson(blk, path) => {
                if let Some(first) = path.last() {
                    match first {
                        JsonNavigationPath::ObjectKey(key) => format!("{}", key).into(),
                        JsonNavigationPath::ListIndex(index) => format!("{}", index).into(),
                    }
                } else {
                    if let Some(blk) = document.vendors().get(*blk) {
                        (&blk.name).into()
                    } else {
                        format!("Vendor block {}", blk).into()
                    }
                }
            }
            Path::RenderPreview => "Test Preview Please Ignore".into(),
        }
    }

    pub fn child_list(&self, document: &Document) -> Option<gio::ListModel> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::Section(Section::PuppetNode) => {
                let root_node = document.puppet_data().nodes().root_node_id;
                let list = gio::ListStore::builder().build();
                list.extend_from_slice(&[Self::new(Path::PuppetNode(root_node))]);

                Some(list.into())
            }
            Path::Section(Section::PuppetParams) => {
                let mut param_paths = vec![];
                for param in document.puppet_data().params.keys() {
                    param_paths.push(Self::new(Path::PuppetParam(param.clone())));
                }

                if param_paths.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(param_paths.as_slice());
                Some(list.into())
            }
            Path::PuppetNode(node_id) => {
                let mut child_node_paths = vec![];
                for child_node in document.puppet_data().nodes().get_children(*node_id) {
                    child_node_paths.push(Self::new(Path::PuppetNode(child_node.uuid)));
                }

                if child_node_paths.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(child_node_paths.as_slice());
                Some(list.into())
            }
            Path::PuppetJson(path) => {
                let value = document.puppet_json.traverse_path(path.as_slice())?;
                let mut children = vec![];

                match value {
                    JsonValue::Object(obj) => {
                        for (key, val) in obj.iter() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonNavigationPath::ObjectKey(key.to_string()));
                            children.push(Self::new(Path::PuppetJson(child_path)));
                        }
                    }
                    JsonValue::Array(list) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonNavigationPath::ListIndex(index));
                            children.push(Self::new(Path::PuppetJson(child_path)));
                        }
                    }
                    _ => return None,
                }

                if children.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(children.as_slice());
                Some(list.into())
            }
            Path::VendorJson(block, path) => {
                let value = document
                    .vendors()
                    .get(*block)?
                    .payload
                    .traverse_path(path.as_slice())?;
                let mut children = vec![];

                match value {
                    JsonValue::Object(obj) => {
                        for (key, val) in obj.iter() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonNavigationPath::ObjectKey(key.to_string()));
                            children.push(Self::new(Path::VendorJson(*block, child_path)));
                        }
                    }
                    JsonValue::Array(list) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonNavigationPath::ListIndex(index));
                            children.push(Self::new(Path::VendorJson(*block, child_path)));
                        }
                    }
                    _ => return None,
                }

                if children.len() == 0 {
                    return None;
                }

                let list = gio::ListStore::builder().build();
                list.extend_from_slice(children.as_slice());
                Some(list.into())
            }
            _ => None,
        }
    }

    pub fn child_inspector(&self, document: Arc<Mutex<Document>>) -> gtk4::Widget {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::Section(Section::PuppetMeta) => MetadataInspector::new(document).into(),
            Path::Section(Section::PuppetPhysics) => PhysicsInspector::new(document).into(),
            Path::PuppetNode(node) => NodeInspector::new(document, *node).into(),
            Path::PuppetParam(param) => ParamInspector::new(document, param.clone()).into(),
            Path::PuppetJson(path) => JsonInspector::new_puppet_json(document, path.clone()).into(),
            Path::VendorJson(blk, path) => {
                JsonInspector::new_vendor_json(document, *blk, path.clone()).into()
            }
            Path::RenderPreview => InoxRenderPreview::new(document).into(),
            path => gtk4::Label::builder()
                .label(format!("Not yet implemented: {:?}", path))
                .build()
                .into(),
        }
    }
}
