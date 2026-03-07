use gio;
use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use json::JsonValue;

use crate::document::Document;
use crate::json::JsonValueExt;
use crate::navigation_item::{JsonIndex, JsonIndexItem, JsonPath};
use crate::string_ext::StrExt;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/json_inspector.ui")]
pub struct JsonInspectorImp {
    document: RefCell<Option<(Arc<Mutex<Document>>, JsonPath, gio::ListStore)>>,

    #[template_child]
    column_view: TemplateChild<gtk4::ColumnView>,
    #[template_child]
    key_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    type_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    value_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    selection: TemplateChild<gtk4::SingleSelection>,
}

#[glib::object_subclass]
impl ObjectSubclass for JsonInspectorImp {
    const NAME: &'static str = "PIPuppetJsonInspector";
    type Type = JsonInspector;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for JsonInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for JsonInspectorImp {}

impl BoxImpl for JsonInspectorImp {}

glib::wrapper! {
    pub struct JsonInspector(ObjectSubclass<JsonInspectorImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl JsonInspector {
    pub fn new_puppet_json(document: Arc<Mutex<Document>>, path: Vec<JsonIndex>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((
            document,
            JsonPath::PuppetJson(path),
            gio::ListStore::builder().build(),
        ));
        selfish.bind();

        selfish
    }

    pub fn new_vendor_json(
        document: Arc<Mutex<Document>>,
        vendor_block: u64,
        path: Vec<JsonIndex>,
    ) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((
            document,
            JsonPath::VendorJson(vendor_block, path),
            gio::ListStore::builder().build(),
        ));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let state = self.imp().document.borrow();
        let (document_arc, path, list_store) = state.as_ref().unwrap();
        let document = document_arc.lock().unwrap();
        let (value, path) = path.as_root_and_path(&document);

        let value: Option<&JsonValue> = value;
        let list_store: &gio::ListStore = list_store;

        if let Some(value) = value.and_then(|v: &JsonValue| v.traverse_path(path)) {
            let mut keys = vec![];
            match value {
                JsonValue::Object(obj) => {
                    for (key, _) in obj.iter() {
                        keys.push(JsonIndexItem::new_object_key(key.to_string()));
                    }
                }
                JsonValue::Array(list) => {
                    for (index, _) in list.iter().enumerate() {
                        keys.push(JsonIndexItem::new_list_index(index));
                    }
                }
                _ => {}
            }

            list_store.extend_from_slice(&keys);
            self.imp()
                .selection
                .set_model(Some(&list_store.clone().upcast::<gio::ListModel>()));

            self.imp().key_factory.connect_setup(|_factory, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                list_item.set_child(Some(
                    &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
                ));
            });

            self.imp().key_factory.connect_bind(|_factory, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                let subkey_item = list_item.item().unwrap();
                let subkey = subkey_item.downcast_ref::<JsonIndexItem>().unwrap();
                let label_child = list_item.child().unwrap();
                let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                label.set_label(&match subkey.as_jsonnavpath() {
                    JsonIndex::ObjectKey(key) => key.escape_nulls().into_owned(),
                    JsonIndex::ListIndex(index) => {
                        format!("{}", index)
                    }
                });
            });

            self.imp().type_factory.connect_setup(|_factory, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                list_item.set_child(Some(
                    &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
                ));
            });

            let type_self = self.clone();
            self.imp()
                .type_factory
                .connect_bind(move |_factory, object| {
                    let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                    let subkey_item = list_item.item().unwrap();
                    let subkey = subkey_item.downcast_ref::<JsonIndexItem>().unwrap();
                    let label_child = list_item.child().unwrap();
                    let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                    let state = type_self.imp().document.borrow();
                    let (document_arc, path, _list_store) = state.as_ref().unwrap();
                    let document = document_arc.lock().unwrap();
                    let (value, path) = path.as_root_and_path(&document);

                    let value: Option<&JsonValue> =
                        value.and_then(|value| value.traverse_path(path));

                    label.set_label(
                        match value.and_then(|value: &JsonValue| {
                            value.traverse_path(&[subkey.as_jsonnavpath().clone()])
                        }) {
                            Some(subval) => subval.as_type(),
                            None => "Undefined",
                        },
                    );
                });

            self.imp().value_factory.connect_setup(|_factory, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                list_item.set_child(Some(
                    &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
                ));
            });

            let value_self = self.clone();
            self.imp()
                .value_factory
                .connect_bind(move |_factory, object| {
                    let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                    let subkey_item = list_item.item().unwrap();
                    let subkey = subkey_item.downcast_ref::<JsonIndexItem>().unwrap();
                    let label_child = list_item.child().unwrap();
                    let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                    let state = value_self.imp().document.borrow();
                    let (document_arc, path, _list_store) = state.as_ref().unwrap();
                    let document = document_arc.lock().unwrap();
                    let (value, path) = path.as_root_and_path(&document);

                    let value: Option<&JsonValue> =
                        value.and_then(|value| value.traverse_path(path));

                    label.set_label(&match value.and_then(|value: &JsonValue| {
                        value.traverse_path(&[subkey.as_jsonnavpath().clone()])
                    }) {
                        Some(JsonValue::Array(list)) => {
                            format!("({} item(s)...)", list.len()).to_string()
                        }
                        Some(JsonValue::Boolean(bool)) => format!("{}", bool).to_string(),
                        Some(JsonValue::Null) => "Null".to_string(),
                        Some(JsonValue::Number(num)) => format!("{}", num).to_string(),
                        Some(JsonValue::Object(obj)) => {
                            format!("({} key(s)...)", obj.len()).to_string()
                        }
                        Some(JsonValue::Short(sh)) => sh.as_str().escape_nulls().into_owned(),
                        Some(JsonValue::String(s)) => s.escape_nulls().into_owned(),
                        None => "Undefined".to_string(),
                    });
                });
        }
    }
}
