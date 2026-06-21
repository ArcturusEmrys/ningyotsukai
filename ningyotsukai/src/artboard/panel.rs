use std::cell::RefCell;

use glib::WeakRef;
use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::stage::StageWidget;

struct State {
    document: Document,

    stage: WeakRef<StageWidget>
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/artboard/panel.ui")]
pub struct ArtboardPanelImp {
    state: RefCell<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for ArtboardPanelImp {
    const NAME: &'static str = "NGTArtboardPanel";
    type Type = ArtboardPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ArtboardPanelImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for ArtboardPanelImp {}

impl BoxImpl for ArtboardPanelImp {}

impl ArtboardPanelImp {
}

glib::wrapper! {
    pub struct ArtboardPanel(ObjectSubclass<ArtboardPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl ArtboardPanel {
    pub fn bind(&self, document: Document, stage: StageWidget) {
        *self.imp().state.borrow_mut() = Some(State {
            document,
            stage: stage.downgrade(),
        });
    }
}
