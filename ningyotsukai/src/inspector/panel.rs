use std::cell::RefCell;
use std::collections::HashSet;

use generational_arena::Index;

use glib::subclass::InitializingObject;
use glib::{Properties, WeakRef};
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use ningyo_extensions::WidgetExt2;

use crate::document::Document;
use crate::inspector::puppet::PuppetInspector;
use crate::stage::{StageWidget, StageWidgetExt};

struct State {
    document: Document,

    stage: WeakRef<StageWidget>,

    last_selection: HashSet<Index>,
}

#[derive(CompositeTemplate, Default, Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/inspector/panel.ui")]
#[properties(wrapper_type=InspectorPanel)]
pub struct InspectorPanelImp {
    state: RefCell<Option<State>>,

    #[template_child]
    inspector_content: gtk4::TemplateChild<gtk4::Box>,
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

impl InspectorPanelImp {
    fn selection_changed(&self) {
        let document = self.state.borrow().as_ref().unwrap().document.clone();
        let stage = self
            .state
            .borrow()
            .as_ref()
            .unwrap()
            .stage
            .upgrade()
            .unwrap();

        let selection = stage.selected_puppets();
        if selection == self.state.borrow().as_ref().unwrap().last_selection {
            return;
        }

        self.state.borrow_mut().as_mut().unwrap().last_selection = selection.clone();
        self.inspector_content.clear_children();

        if selection.len() > 1 {
            //Multiselect UI
            let empty = gtk4::Label::builder()
                .label(format!("{} things are selected", selection.len()))
                .build();
            self.inspector_content.append(&empty);
        } else if let Some(select) = selection.iter().next() {
            let puppet_inspect = PuppetInspector::new(document, *select, stage);
            self.inspector_content.append(&puppet_inspect);
        } else {
            //Empty select UI
            let empty = gtk4::Label::builder().label("Nothing is selected").build();
            self.inspector_content.append(&empty);
        }
    }
}

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
            last_selection: HashSet::new(),
        });

        self.imp().selection_changed();

        stage.connect_selection_changed({
            let stage_self = self.downgrade();
            move |_| {
                if let Some(stage_self) = stage_self.upgrade() {
                    stage_self.imp().selection_changed();
                }
            }
        });
    }
}
