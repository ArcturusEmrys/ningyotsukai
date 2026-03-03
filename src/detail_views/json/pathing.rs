use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use json::{JsonValue, object::Object as JsonObject};

use crate::document::Document;
use crate::navigation_item::JsonNavigationPath;
use crate::string_ext::StrExt;

pub enum JsonPath {
    PuppetJson(Vec<JsonNavigationPath>),
    VendorJson(usize, Vec<JsonNavigationPath>),
}

pub trait JsonValueExt {
    fn as_object(&self) -> Option<&JsonObject>;
    fn as_list(&self) -> Option<&[JsonValue]>;
    fn traverse_path<'a>(&'a self, path: &[JsonNavigationPath]) -> Option<&'a JsonValue>;
    fn as_type(&self) -> &'static str;
}

impl JsonValueExt for JsonValue {
    fn as_object(&self) -> Option<&JsonObject> {
        match self {
            JsonValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(list) => Some(list.as_slice()),
            _ => None,
        }
    }

    fn as_type(&self) -> &'static str {
        match self {
            JsonValue::Null => "Null",
            JsonValue::Object(_) => "Object",
            JsonValue::Short(_) => "String",
            JsonValue::String(_) => "String",
            JsonValue::Number(_) => "Number",
            JsonValue::Boolean(_) => "Bool",
            JsonValue::Array(_) => "Array",
        }
    }

    fn traverse_path<'a>(&'a self, path: &[JsonNavigationPath]) -> Option<&'a JsonValue> {
        let mut value = self;
        for component in path {
            match component {
                JsonNavigationPath::ObjectKey(key) => {
                    let obj = value.as_object()?;

                    value = obj.get(key)?;
                }
                JsonNavigationPath::ListIndex(index) => {
                    let list = value.as_list()?;

                    value = list.get(*index)?;
                }
            }
        }

        Some(value)
    }
}
