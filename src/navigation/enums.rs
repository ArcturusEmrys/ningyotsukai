use inox2d;
use json::JsonValue;
use std::fmt::Debug;

use crate::document::Document;

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
#[derive(Clone, Debug, glib::Variant, PartialEq, Eq)]
pub enum Path {
    Section(Section),
    PuppetNode(InoxNodeUuid),
    PuppetParam(ParamUuid),
    PuppetJson(Vec<JsonIndex>),
    VendorJson(u64, Vec<JsonIndex>),
    RenderPreview,
}

impl Path {
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
            Path::RenderPreview => None,
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
