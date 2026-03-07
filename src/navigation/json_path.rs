use glib::subclass::InitializingObject;
use glib::subclass::prelude::*;

use std::cell::RefCell;

use crate::navigation::enums::{JsonIndex, JsonPath};

#[derive(Default)]
pub struct JsonPathItemImp(RefCell<Option<JsonPath>>);

#[glib::object_subclass]
impl ObjectSubclass for JsonPathItemImp {
    const NAME: &'static str = "PIJsonPathItem";
    type Type = JsonPathItem;
    type ParentType = glib::Object;

    fn class_init(_class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for JsonPathItemImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

glib::wrapper! {
    pub struct JsonPathItem(ObjectSubclass<JsonPathItemImp>);
}

impl JsonPathItem {
    pub fn new_puppet_path(path: &[JsonIndex]) -> Self {
        let selfish: JsonPathItem = glib::Object::builder().build();

        *selfish.imp().0.borrow_mut() = Some(JsonPath::PuppetJson(path.to_vec()));

        selfish
    }

    pub fn new_vendor_path(vendor_index: usize, path: &[JsonIndex]) -> Self {
        let selfish: JsonPathItem = glib::Object::builder().build();

        *selfish.imp().0.borrow_mut() =
            Some(JsonPath::VendorJson(vendor_index as u64, path.to_vec()));

        selfish
    }

    pub fn as_json_path(&self) -> JsonPath {
        self.imp().0.borrow().as_ref().unwrap().clone()
    }
}
