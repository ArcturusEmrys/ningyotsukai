use glam::Vec2;
use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::render_preview::param::RenderParam;
use ningyo_extensions::JsonValueExt;

struct State {
    document: Arc<Mutex<Document>>,

    param_overrides: HashMap<String, Vec2>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/render_preview/param_list.ui")]
pub struct RenderParamListImp {
    state: RefCell<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for RenderParamListImp {
    const NAME: &'static str = "PIRenderParamList";
    type Type = RenderParamList;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for RenderParamListImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for RenderParamListImp {}

impl BoxImpl for RenderParamListImp {}

glib::wrapper! {
    pub struct RenderParamList(ObjectSubclass<RenderParamListImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl RenderParamList {
    pub fn bind(&self, document: Arc<Mutex<Document>>) {
        let mut names = vec![];
        {
            let document_guard = document.lock().unwrap();
            let params_json = document_guard
                .puppet_json
                .as_object()
                .unwrap()
                .get("param")
                .unwrap()
                .as_list()
                .unwrap();
            for param_json in params_json.iter() {
                let name = param_json
                    .as_object()
                    .unwrap()
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap();
                names.push(name.to_string());
            }
        }

        for name in names {
            self.append(&RenderParam::new(document.clone(), &name));
        }

        *self.imp().state.borrow_mut() = Some(State {
            document,
            param_overrides: HashMap::new(),
        });
    }

    /// Queue a parameter override to be set at the start of the next frame.
    pub fn set_override(&self, param: &str, value: Vec2) {
        self.imp()
            .state
            .borrow_mut()
            .as_mut()
            .unwrap()
            .param_overrides
            .insert(param.to_string(), value);
    }

    /// Called at the start of a frame to set all parameters that have overrides.
    pub fn do_param_set(&self, document_borrow: &mut Document) {
        let borrow = self.imp().state.borrow();
        let param_overrides = &borrow.as_ref().unwrap().param_overrides;
        for (param, value) in param_overrides.iter() {
            document_borrow
                .model
                .puppet
                .param_ctx
                .as_mut()
                .unwrap()
                .set(param, *value)
                .unwrap();
        }
    }
}
