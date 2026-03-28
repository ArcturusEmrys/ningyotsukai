//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use generational_arena::Index;
use glam::Vec2;

use ningyo_extensions::prelude::*;

use crate::document::Document;
use crate::stage::StageWidget;

struct PuppetBoundsGizmoState {
    /// The document this gizmo's puppet is from.
    document: Arc<Mutex<Document>>,

    /// The puppet we're tracking.
    puppet: Index,
}

#[derive(Default)]
pub struct PuppetBoundsGizmoImp {
    state: RefCell<Option<PuppetBoundsGizmoState>>,
}

#[glib::object_subclass]
impl ObjectSubclass for PuppetBoundsGizmoImp {
    const NAME: &'static str = "NGTPuppetBoundsGizmo";
    type Type = PuppetBoundsGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-puppet");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for PuppetBoundsGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();

        let drag = gtk4::GestureDrag::builder()
            .button(gdk4::BUTTON_PRIMARY)
            .build();

        drag.connect_drag_begin(|drag, _, _| {
            drag.set_state(gtk4::EventSequenceState::Claimed);
        });

        let drag_end_self = self.obj().clone();
        drag.connect_drag_update(move |_, now_x, now_y| {
            let mut state = drag_end_self.imp().state.borrow_mut();
            let state = state.as_mut().unwrap();
            let mut document = state.document.lock().unwrap();
            let puppet = document.stage_mut().puppet_mut(state.puppet);

            if let Some(puppet) = puppet {
                let delta = Vec2::new(now_x as f32, now_y as f32);
                let rune = puppet.position();

                puppet.set_position(delta + rune);
            }

            if let Some(stage) = drag_end_self.closest::<StageWidget>() {
                // If I do this on the same stack the app hangs
                glib::timeout_add_local(Duration::new(0, 0), move || {
                    stage.puppet_updated();
                    glib::ControlFlow::Break
                });
            }
        });

        self.obj().add_controller(drag);
        self.obj().set_size_request(0, 0);
    }
}

impl WidgetImpl for PuppetBoundsGizmoImp {}

impl ScrollableImpl for PuppetBoundsGizmoImp {}

glib::wrapper! {
    pub struct PuppetBoundsGizmo(ObjectSubclass<PuppetBoundsGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl PuppetBoundsGizmo {
    pub fn new(document: Arc<Mutex<Document>>, puppet: Index) -> Self {
        let gizmo: Self = glib::Object::builder().build();

        gizmo.set_cursor(gdk4::Cursor::from_name("grab", None).as_ref());

        {
            let mut state = gizmo.imp().state.borrow_mut();

            *state = Some(PuppetBoundsGizmoState { document, puppet });
        }

        gizmo
    }
}
