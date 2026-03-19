use json::{JsonValue, object::Object as JsonObject};

use crate::navigation::JsonIndex;

pub trait JsonValueExt {
    fn as_object(&self) -> Option<&JsonObject>;
    fn as_list(&self) -> Option<&[JsonValue]>;
    fn traverse_path<'a>(&'a self, path: &[JsonIndex]) -> Option<&'a JsonValue>;
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

    fn traverse_path<'a>(&'a self, path: &[JsonIndex]) -> Option<&'a JsonValue> {
        let mut value = self;
        for component in path {
            match component {
                JsonIndex::ObjectKey(key) => {
                    let obj = value.as_object()?;

                    value = obj.get(key)?;
                }
                JsonIndex::ListIndex(index) => {
                    let list = value.as_list()?;

                    value = list.get(*index as usize)?;
                }
            }
        }

        Some(value)
    }
}
