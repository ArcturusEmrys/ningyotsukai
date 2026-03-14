use inox2d;
use json::JsonValue;
use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use crate::detail_views::{
    JsonInspector, MetadataInspector, NodeInspector, NodeSearch, ParamInspector, ParamSearch,
    PhysicsInspector, TextureBrowser,
};
use crate::document::Document;
use crate::json::JsonValueExt;

/// An instruction on how to traverse a JSON object.
#[derive(Debug, Clone, PartialEq, Eq, glib::Variant)]
pub enum JsonIndex {
    ObjectKey(String),
    ListIndex(u64),
}

/// A reference to a specific JSON object within an Inochi puppet file.
#[derive(Debug, Clone, PartialEq, Eq, glib::Variant)]
pub enum JsonPath {
    PuppetJson(Vec<JsonIndex>),
    VendorJson(u64, Vec<JsonIndex>),
}

impl JsonPath {
    pub fn as_path(&self) -> &[JsonIndex] {
        match self {
            Self::PuppetJson(path) => path.as_slice(),
            Self::VendorJson(_, path) => path.as_slice(),
        }
    }

    pub fn as_root_and_path<'a, 'doc>(
        &'a self,
        document: &'doc Document,
    ) -> (Option<&'doc JsonValue>, &'a [JsonIndex]) {
        match self {
            JsonPath::PuppetJson(path) => (Some(&document.puppet_json), path),
            JsonPath::VendorJson(block, path) => (
                document.vendors().get(*block as usize).map(|v| &v.payload),
                path,
            ),
        }
    }

    pub fn with_object_key(self, key: &str) -> Self {
        match self {
            JsonPath::PuppetJson(mut path) => {
                path.push(JsonIndex::ObjectKey(key.to_string()));
                Self::PuppetJson(path)
            }
            JsonPath::VendorJson(block, mut path) => {
                path.push(JsonIndex::ObjectKey(key.to_string()));
                Self::VendorJson(block, path)
            }
        }
    }

    pub fn with_list_index(self, index: u64) -> Self {
        match self {
            JsonPath::PuppetJson(mut path) => {
                path.push(JsonIndex::ListIndex(index));
                Self::PuppetJson(path)
            }
            JsonPath::VendorJson(block, mut path) => {
                path.push(JsonIndex::ListIndex(index));
                Self::VendorJson(block, path)
            }
        }
    }
}

/// A specific section of the detail pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, glib::Variant)]
pub enum Section {
    PuppetMeta,
    PuppetPhysics,
    PuppetNode,
    PuppetParams,
    ModelTextures,
    VendorData,
}

/// Shim type for serializing Inox2D node UUIDs into a GVariant.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, glib::Variant)]
#[repr(transparent)]
pub struct InoxNodeUuid(u32);

impl Into<inox2d::node::InoxNodeUuid> for InoxNodeUuid {
    fn into(self) -> inox2d::node::InoxNodeUuid {
        // SAFETY: We have the same representation and validity as Inox2D's node type.
        unsafe { std::mem::transmute(self) }
    }
}

impl From<inox2d::node::InoxNodeUuid> for InoxNodeUuid {
    fn from(node: inox2d::node::InoxNodeUuid) -> Self {
        Self(node.into())
    }
}

/// Shim type for serializing Inox2D param UUIDs into a GVariant.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, glib::Variant)]
#[repr(transparent)]
pub struct ParamUuid(pub u32);

impl Into<inox2d::params::ParamUuid> for ParamUuid {
    fn into(self) -> inox2d::params::ParamUuid {
        inox2d::params::ParamUuid(self.0)
    }
}

impl From<inox2d::params::ParamUuid> for ParamUuid {
    fn from(param: inox2d::params::ParamUuid) -> Self {
        Self(param.0)
    }
}

/// A specific detail page in the app.
///
/// Detail pages exist in a hierarchy of pages and this structure is
/// responsible for keeping navigation in that hierarchy consistent. Some
/// navigation actions may require referencing the associated document.
///
/// When adding a new page in the app, think about:
///
/// 1. What's the page's name?
/// 2. Where will that page live?
/// 3. What will be its parents and children?
///
/// Some detail pages are not intended to actually be viewed. For example, you
/// might need to populate a GTK list model with references to specific parts
/// of the document, but not care about actually navigating to them. That use
/// case is also specifically supported.
///
/// This struct has a GObject wrapper `NavigationItem` and may also be
/// serialized into a GVariant.
#[derive(Clone, Debug, glib::Variant, PartialEq, Eq)]
pub enum Path {
    Section(Section),
    PuppetNode(InoxNodeUuid),
    PuppetParam(ParamUuid),
    PuppetParamBinding(ParamUuid, u64),
    ModelTexture(u64),
    PuppetJson(Vec<JsonIndex>),
    VendorJson(u64, Vec<JsonIndex>),
}

impl Path {
    /// What's the page's name?
    pub fn name<'a>(&self, document: &'a Document) -> Cow<'a, str> {
        match self {
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
            Path::PuppetParam(uuid) => {
                if let Some((name, _)) = document
                    .model
                    .puppet
                    .params
                    .iter()
                    .find(|(_k, v)| v.uuid.0 == uuid.0)
                {
                    (name).into()
                } else {
                    "<MISSING OR INVALID PARAM>".into()
                }
            }
            Path::PuppetParamBinding(uuid, index) => {
                for (_, param) in document.model.puppet.params.iter() {
                    if param.uuid.0 == uuid.0 {
                        for (bind_index, binding) in param.bindings.iter().enumerate() {
                            if bind_index as u64 == *index {
                                let binding_path = self.as_json_path(document).unwrap();
                                let binding_json =
                                    document.puppet_json.traverse_path(binding_path.as_path());
                                let binding_kind = binding_json
                                    .and_then(|v| v.as_object())
                                    .and_then(|o| o.get("param_name"))
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("<INVALID BINDING TARGET>");

                                return format!(
                                    "{}.{}",
                                    document
                                        .model
                                        .puppet
                                        .nodes()
                                        .get_node(binding.node)
                                        .map(|n| n.name.as_str())
                                        .unwrap_or("<INVALID BOUND NODE>"),
                                    binding_kind
                                )
                                .into();
                            }
                        }
                    }
                }

                "<MISSING OR INVALID BINDING>".into()
            }
            Path::ModelTexture(tex) => format!("Model texture {}", tex).into(),
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
        }
    }

    /// What notebook page of the app does this navigation item live in?
    ///
    /// This should be self-consistent with the path's `parent`, if one is
    /// available.
    pub fn notebook_page(&self) -> u32 {
        match self {
            Path::Section(_)
            | Path::PuppetNode(_)
            | Path::PuppetParam(_)
            | Path::PuppetParamBinding(_, _)
            | Path::ModelTexture(_) => 0,
            Path::PuppetJson(_) | Path::VendorJson(_, _) => 1,
        }
    }

    /// What is the item's intended parent (if any)?
    pub fn parent(&self, document: &Document) -> Option<Path> {
        match self {
            Path::Section(_) => None,
            Path::PuppetNode(node_uuid)
                if <InoxNodeUuid as Into<inox2d::node::InoxNodeUuid>>::into(*node_uuid)
                    != document.model.puppet.nodes().root_node_id.into() =>
            {
                Some(Path::PuppetNode(
                    document
                        .model
                        .puppet
                        .nodes()
                        .get_parent((*node_uuid).into())
                        .uuid
                        .into(),
                ))
            }
            // Root Inox node
            Path::PuppetNode(_node_uuid) => Some(Path::Section(Section::PuppetNode)),
            Path::PuppetParam(_) => Some(Path::Section(Section::PuppetParams)),
            Path::PuppetParamBinding(param, _) => Some(Path::PuppetParam(*param)),
            Path::ModelTexture(_) => Some(Path::Section(Section::ModelTextures)),
            Path::PuppetJson(json_path) if json_path.len() > 0 => {
                Some(Path::PuppetJson(json_path[0..json_path.len() - 1].to_vec()))
            }
            // Root Puppet JSON
            Path::PuppetJson(_) => None,
            Path::VendorJson(index, json_path) if json_path.len() > 0 => Some(Path::VendorJson(
                *index,
                json_path[0..json_path.len() - 1].to_vec(),
            )),
            // Root Vendor JSON
            Path::VendorJson(_, _) => None,
        }
    }

    /// Can I see this node's JSON (and if so, how do I get there?)
    pub fn as_json_path(&self, document: &Document) -> Option<JsonPath> {
        match self {
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
                let mut current_node_id: u32 = current_node.0;

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
                            <inox2d::node::InoxNodeUuid as Into<InoxNodeUuid>>::into(child.uuid).0
                                == current_node_id
                        })
                        .expect("valid child");

                    reverse_parent_path.push(JsonIndex::ListIndex(child_index as u64));
                    reverse_parent_path.push(JsonIndex::ObjectKey("children".to_string()));

                    current_node = parent_id.into();
                    current_node_id = current_node.0;
                }

                reverse_parent_path.push(JsonIndex::ObjectKey("nodes".to_string()));
                reverse_parent_path.reverse();

                Some(JsonPath::PuppetJson(reverse_parent_path))
            }
            Path::PuppetParam(uuid) => {
                let params_list = document.puppet_json.as_object()?.get("param")?.as_list()?;

                for (index, val) in params_list.iter().enumerate() {
                    if let Some(obj) = val.as_object() {
                        if let Some(obj_uuid) = obj.get("uuid").and_then(|v| v.as_u32()) {
                            if obj_uuid == uuid.0 {
                                return Some(JsonPath::PuppetJson(vec![
                                    JsonIndex::ObjectKey("param".to_string()),
                                    JsonIndex::ListIndex(index as u64),
                                ]));
                            }
                        }
                    }
                }

                None
            }
            Path::PuppetParamBinding(_uuid, bind_index) => Some(
                self.parent(document)
                    .unwrap()
                    .as_json_path(document)
                    .unwrap()
                    .with_object_key("bindings")
                    .with_list_index(*bind_index),
            ),
            Path::ModelTexture(_) => None,
        }
    }

    /// What children does this node have?
    pub fn child_list(&self, document: &Document) -> Vec<Path> {
        match self {
            Path::Section(Section::PuppetNode) => {
                let root_node = document.puppet_data().nodes().root_node_id;
                vec![Path::PuppetNode(root_node.into())]
            }
            Path::Section(Section::PuppetParams) => {
                let mut param_paths = vec![];
                for param in document.puppet_data().params.values() {
                    param_paths.push(Path::PuppetParam(param.uuid.into()));
                }

                param_paths
            }
            Path::PuppetNode(node_id) => {
                let mut child_node_paths = vec![];
                for child_node in document
                    .puppet_data()
                    .nodes()
                    .get_children((*node_id).into())
                {
                    child_node_paths.push(Path::PuppetNode(child_node.uuid.into()));
                }

                child_node_paths
            }
            Path::PuppetJson(path) => {
                let mut children = vec![];

                match document.puppet_json.traverse_path(path.as_slice()) {
                    Some(JsonValue::Object(obj)) => {
                        for (key, val) in obj.iter() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ObjectKey(key.to_string()));
                            children.push(Path::PuppetJson(child_path));
                        }
                    }
                    Some(JsonValue::Array(list)) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ListIndex(index as u64));
                            children.push(Path::PuppetJson(child_path));
                        }
                    }
                    _ => {}
                }

                children
            }
            Path::VendorJson(block, path) => {
                let mut children = vec![];

                match document
                    .vendors()
                    .get(*block as usize)
                    .and_then(|v| v.payload.traverse_path(path.as_slice()))
                {
                    Some(JsonValue::Object(obj)) => {
                        for (key, val) in obj.iter() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ObjectKey(key.to_string()));
                            children.push(Path::VendorJson(*block, child_path));
                        }
                    }
                    Some(JsonValue::Array(list)) => {
                        for (index, val) in list.iter().enumerate() {
                            if !val.is_object() && !val.is_array() {
                                continue;
                            }

                            let mut child_path = path.clone();
                            child_path.push(JsonIndex::ListIndex(index as u64));
                            children.push(Path::VendorJson(*block, child_path));
                        }
                    }
                    _ => {}
                }

                children
            }
            _ => vec![],
        }
    }

    pub fn child_inspector(&self, document: Arc<Mutex<Document>>) -> gtk4::Widget {
        match self {
            Path::Section(Section::PuppetMeta) => MetadataInspector::new(document).into(),
            Path::Section(Section::PuppetPhysics) => PhysicsInspector::new(document).into(),
            Path::Section(Section::PuppetNode) => NodeSearch::new(document).into(),
            Path::Section(Section::PuppetParams) => ParamSearch::new(document).into(),
            Path::Section(Section::ModelTextures) => TextureBrowser::new(document).into(),
            Path::PuppetNode(node) => NodeInspector::new(document, (*node).into()).into(),
            Path::PuppetParam(param) => ParamInspector::new(document, param.clone().into()).into(),
            Path::PuppetJson(path) => JsonInspector::new_puppet_json(document, path.clone()).into(),
            Path::VendorJson(blk, path) => {
                JsonInspector::new_vendor_json(document, *blk, path.clone()).into()
            }
            path => gtk4::Label::builder()
                .label(format!("Not yet implemented: {:?}", path))
                .build()
                .into(),
        }
    }
}

impl From<JsonPath> for Path {
    fn from(json_path: JsonPath) -> Self {
        match json_path {
            JsonPath::PuppetJson(path) => Path::PuppetJson(path),
            JsonPath::VendorJson(ind, path) => Path::VendorJson(ind, path),
        }
    }
}
