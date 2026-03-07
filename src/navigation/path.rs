use glib;
use gtk4::subclass::prelude::*;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use json::JsonValue;

use inox2d::node::InoxNodeUuid;

use crate::detail_views::{
    InoxRenderPreview, JsonInspector, MetadataInspector, NodeInspector, NodeSearch, ParamInspector,
    PhysicsInspector,
};
use crate::document::Document;
use crate::json::JsonValueExt;
use crate::navigation::enums::{JsonIndex, JsonPath, Path, Section};

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

    pub fn from_node(node_id: InoxNodeUuid) -> Self {
        Self::new(Path::PuppetNode(node_id.into()))
    }

    pub fn as_path(&self) -> Path {
        self.imp().path.borrow().as_ref().expect("a path").clone()
    }

    pub fn as_puppet_node(&self) -> Option<InoxNodeUuid> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::PuppetNode(node_id) => Some((*node_id).into()),
            _ => None,
        }
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
                let node = document.puppet_data().nodes().get_node((*node_id).into());

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
                        JsonIndex::ObjectKey(key) => format!("{}", key).into(),
                        JsonIndex::ListIndex(index) => format!("{}", index).into(),
                    }
                } else {
                    format!("Puppet JSON").into()
                }
            }
            Path::VendorJson(blk, path) => {
                if let Some(first) = path.last() {
                    match first {
                        JsonIndex::ObjectKey(key) => format!("{}", key).into(),
                        JsonIndex::ListIndex(index) => format!("{}", index).into(),
                    }
                } else {
                    if let Some(blk) = document.vendors().get(*blk as usize) {
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
                list.extend_from_slice(&[Self::new(Path::PuppetNode(root_node.into()))]);

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
                for child_node in document
                    .puppet_data()
                    .nodes()
                    .get_children((*node_id).into())
                {
                    child_node_paths.push(Self::new(Path::PuppetNode(child_node.uuid.into())));
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
                            child_path.push(JsonIndex::ObjectKey(key.to_string()));
                            children.push(Self::new(Path::PuppetJson(child_path)));
                        }
                    }
                    JsonValue::Array(list) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ListIndex(index as u64));
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
                    .get(*block as usize)?
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
                            child_path.push(JsonIndex::ObjectKey(key.to_string()));
                            children.push(Self::new(Path::VendorJson(*block, child_path)));
                        }
                    }
                    JsonValue::Array(list) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ListIndex(index as u64));
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
            Path::Section(Section::PuppetNode) => NodeSearch::new(document).into(),
            Path::PuppetNode(node) => NodeInspector::new(document, (*node).into()).into(),
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

    pub fn notebook_page(&self) -> u32 {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::Section(_) | Path::PuppetNode(_) | Path::PuppetParam(_) => 0,
            Path::PuppetJson(_) | Path::VendorJson(_, _) => 1,
            Path::RenderPreview => 2,
        }
    }

    pub fn as_json_path(&self, document: &Document) -> Option<JsonPath> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::Section(Section::PuppetMeta) => {
                Some(JsonPath::PuppetJson(vec![JsonIndex::ObjectKey(
                    "meta".to_string(),
                )]))
            }
            Path::Section(Section::PuppetPhysics) => {
                Some(JsonPath::PuppetJson(vec![JsonIndex::ObjectKey(
                    "physics".to_string(),
                )]))
            }
            Path::Section(Section::PuppetNode) => {
                Some(JsonPath::PuppetJson(vec![JsonIndex::ObjectKey(
                    "nodes".to_string(),
                )]))
            }
            Path::Section(Section::PuppetParams) => {
                Some(JsonPath::PuppetJson(vec![JsonIndex::ObjectKey(
                    "param".to_string(),
                )]))
            }
            Path::Section(Section::ModelTextures) => None,
            Path::Section(Section::VendorData) => None,
            Path::PuppetJson(path) => Some(JsonPath::PuppetJson(path.to_vec())),
            Path::VendorJson(index, path) => Some(JsonPath::VendorJson(*index, path.to_vec())),
            Path::PuppetNode(node) => {
                let root_node: u32 = document.model.puppet.nodes().root_node_id.into();
                let mut current_node: InoxNodeUuid = (*node).into();
                let mut current_node_id: u32 = current_node.into();

                let mut reverse_parent_path = vec![];
                while current_node_id != root_node {
                    let parent_id = document
                        .model
                        .puppet
                        .nodes()
                        .get_parent(current_node.into())
                        .uuid;
                    let (child_index, _) = document
                        .model
                        .puppet
                        .nodes()
                        .get_children(parent_id)
                        .enumerate()
                        .find(|(_i, child)| {
                            <InoxNodeUuid as Into<u32>>::into(child.uuid) == current_node_id
                        })
                        .expect("valid child");

                    reverse_parent_path.push(JsonIndex::ListIndex(child_index as u64));
                    reverse_parent_path.push(JsonIndex::ObjectKey("children".to_string()));

                    current_node = parent_id;
                    current_node_id = current_node.into();
                }

                reverse_parent_path.push(JsonIndex::ObjectKey("nodes".to_string()));
                reverse_parent_path.reverse();

                Some(JsonPath::PuppetJson(reverse_parent_path))
            }
            Path::PuppetParam(_name) => None, //TODO: Unimplemented!
            Path::RenderPreview => Some(JsonPath::PuppetJson(vec![])),
        }
    }
}
