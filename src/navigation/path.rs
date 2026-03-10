use glib;
use gtk4::subclass::prelude::*;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::node::InoxNodeUuid;
use inox2d::params::ParamUuid;

use crate::document::Document;
use crate::navigation::enums::{JsonPath, Path};

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

    pub fn from_param(param_id: ParamUuid) -> Self {
        Self::new(Path::PuppetParam(param_id.into()))
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

    pub fn as_puppet_param(&self) -> Option<ParamUuid> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::PuppetParam(param_id) => Some((*param_id).into()),
            _ => None,
        }
    }

    pub fn as_puppet_param_binding(&self) -> Option<(ParamUuid, usize)> {
        match self.imp().path.borrow().as_ref().expect("a path") {
            Path::PuppetParamBinding(param_id, index) => {
                Some(((*param_id).into(), *index as usize))
            }
            _ => None,
        }
    }

    pub fn name<'a>(&self, document: &'a Document) -> Cow<'a, str> {
        self.imp()
            .path
            .borrow()
            .as_ref()
            .expect("a path")
            .name(document)
    }

    pub fn child_list(&self, document: &Document) -> Option<gio::ListModel> {
        let children = self
            .imp()
            .path
            .borrow()
            .as_ref()
            .expect("a path")
            .child_list(document);

        if children.len() == 0 {
            None
        } else {
            let list = gio::ListStore::builder().build();
            let wrapped_children: Vec<Self> = children.into_iter().map(|c| Self::new(c)).collect();
            list.extend_from_slice(wrapped_children.as_slice());

            Some(list.into())
        }
    }

    pub fn child_inspector(&self, document: Arc<Mutex<Document>>) -> gtk4::Widget {
        self.imp()
            .path
            .borrow()
            .as_ref()
            .expect("a path")
            .child_inspector(document)
    }

    pub fn notebook_page(&self) -> u32 {
        self.imp()
            .path
            .borrow()
            .as_ref()
            .expect("a path")
            .notebook_page()
    }

    pub fn as_json_path(&self, document: &Document) -> Option<JsonPath> {
        self.imp()
            .path
            .borrow()
            .as_ref()
            .expect("a path")
            .as_json_path(document)
    }
}
