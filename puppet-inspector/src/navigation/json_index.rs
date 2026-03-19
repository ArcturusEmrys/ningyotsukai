use glib::subclass::InitializingObject;
use glib::subclass::prelude::*;

use std::cell::RefCell;

use crate::navigation::JsonIndex;

#[derive(Default)]
pub struct JsonIndexItemImp(RefCell<Option<JsonIndex>>);

#[glib::object_subclass]
impl ObjectSubclass for JsonIndexItemImp {
    const NAME: &'static str = "PIJsonIndexItem";
    type Type = JsonIndexItem;
    type ParentType = glib::Object;

    fn class_init(_class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for JsonIndexItemImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

glib::wrapper! {
    pub struct JsonIndexItem(ObjectSubclass<JsonIndexItemImp>);
}

impl JsonIndexItem {
    pub fn new_object_key(key: String) -> Self {
        let selfish: JsonIndexItem = glib::Object::builder().build();

        *selfish.imp().0.borrow_mut() = Some(JsonIndex::ObjectKey(key));

        selfish
    }

    pub fn new_list_index(index: usize) -> Self {
        let selfish: JsonIndexItem = glib::Object::builder().build();

        *selfish.imp().0.borrow_mut() = Some(JsonIndex::ListIndex(index as u64));

        selfish
    }

    pub fn as_jsonnavpath(&self) -> JsonIndex {
        self.imp().0.borrow().as_ref().unwrap().clone()
    }
}
