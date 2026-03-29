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
use glam::Vec3Swizzles;

use ningyo_extensions::prelude::*;

use crate::document::Document;
use crate::stage::StageWidget;
use crate::stage::gizmos::origin::PuppetOriginGizmo;

struct PuppetBoundsGizmoState {
    /// The document this gizmo's puppet is from.
    document: Arc<Mutex<Document>>,

    /// The puppet we're tracking.
    puppet: Index,

    /// A gizmo to render the puppet's origin with.
    origin: PuppetOriginGizmo,
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
            let index = state.puppet;
            let puppet = document.stage_mut().puppet_mut(index);

            if let Some(puppet) = puppet {
                let delta = Vec2::new(now_x as f32, now_y as f32);
                let rune = puppet.position();

                puppet.set_position(delta + rune);
            }

            if let Some(stage) = drag_end_self.closest::<StageWidget>() {
                stage.set_selected_puppet(Some(index));
                // If I do this on the same stack the app hangs
                glib::timeout_add_local(Duration::new(0, 0), move || {
                    stage.puppet_updated();
                    glib::ControlFlow::Break
                });
            }
        });

        self.obj().add_controller(drag);

        let select = gtk4::GestureClick::builder()
            .button(gdk4::BUTTON_SECONDARY)
            .build();
        let select_pressed_self = self.obj().clone();
        select.connect_pressed(move |_, _, _, _| {
            let state = select_pressed_self.imp().state.borrow();
            let index = state.as_ref().unwrap().puppet;

            drop(state);
            select_pressed_self
                .closest::<StageWidget>()
                .unwrap()
                .set_selected_puppet(Some(index));
        });
        self.obj().add_controller(select);

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

        let subgizmo = PuppetOriginGizmo::new();
        subgizmo.set_parent(&gizmo);

        {
            let mut state = gizmo.imp().state.borrow_mut();

            *state = Some(PuppetBoundsGizmoState {
                document,
                puppet,
                origin: subgizmo,
            });
        }

        gizmo
    }

    /// Called whenever the associated puppet has changed.
    pub fn puppet_updated(&self, stage: &StageWidget) {
        let state = self.imp().state.borrow_mut();
        let state = state.as_ref().unwrap();
        let document_arc = state.document.clone();
        let document = document_arc.lock().unwrap();
        let puppet = document.stage().puppet(state.puppet).unwrap();

        if let Some(bounds) = puppet.bounds() {
            let bounds_tl = bounds.top_left_point();
            let bounds_br = bounds.bottom_right_point();

            let bounds_width = bounds_br.x - bounds_tl.x;
            let bounds_height = bounds_br.y - bounds_tl.y;

            let root_offset = puppet
                .model()
                .puppet
                .nodes()
                .get_node(puppet.model().puppet.nodes().root_node_id)
                .unwrap()
                .trans_offset
                .translation
                .xy();
            let offset = puppet.position();

            let viewport_tl = stage.project_stage_to_viewport(bounds_tl + offset + root_offset);
            let viewport_br = stage.project_stage_to_viewport(bounds_br + offset + root_offset);

            let width = viewport_br.x - viewport_tl.x;
            let height = viewport_br.y - viewport_tl.y;

            self.set_visible(true);
            self.measure(gtk4::Orientation::Horizontal, bounds.width() as i32);
            self.allocate(
                width as i32,
                height as i32,
                -1,
                Some(
                    gsk4::Transform::new()
                        .translate(&graphene::Point::new(viewport_tl.x, viewport_tl.y)),
                ),
            );

            let origin_scale_x = bounds_width / width;
            let origin_scale_y = bounds_height / height;

            let origin_x = (0.0 - bounds_tl.x) / origin_scale_x;
            let origin_y = (0.0 - bounds_tl.y) / origin_scale_y;

            state.origin.set_visible(true);
            state.origin.measure(gtk4::Orientation::Horizontal, 10);
            state.origin.allocate(
                3,
                3,
                -1,
                Some(gsk4::Transform::new().translate(&graphene::Point::new(origin_x, origin_y))),
            );
        } else {
            //TODO: Strictly speaking, this is an error state.
            //Nobody is going to make an empty puppet, so we should do... something?! reasonable?!?!
            self.set_visible(false);
            state.origin.set_visible(false);
        }
    }
}
