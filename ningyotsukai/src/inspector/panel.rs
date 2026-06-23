use std::cell::RefCell;
use std::mem::swap;
use std::str::FromStr;

use glib::subclass::InitializingObject;
use glib::{Properties, WeakRef};
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::stage::StageWidget;

struct State {
    document: Document,

    stage: WeakRef<StageWidget>,
}

#[derive(CompositeTemplate, Default, Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/inspector/panel.ui")]
#[properties(wrapper_type=InspectorPanel)]
pub struct InspectorPanelImp {
    state: RefCell<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for InspectorPanelImp {
    const NAME: &'static str = "NGTInspectorPanel";
    type Type = InspectorPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for InspectorPanelImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for InspectorPanelImp {}

impl BoxImpl for InspectorPanelImp {}

glib::wrapper! {
    pub struct InspectorPanel(ObjectSubclass<InspectorPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl InspectorPanel {
    pub fn bind(&self, document: Document, stage: StageWidget) {
        *self.imp().state.borrow_mut() = Some(State {
            document,
            stage: stage.downgrade(),
        });
    }
}
