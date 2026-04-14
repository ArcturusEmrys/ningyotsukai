//! A Gizmo-style class that manages the bounds of an active stage selection.
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use generational_arena::Index;

use crate::document::Document;
use crate::stage::StageWidget;

use ningyo_extensions::prelude::*;

#[derive(Default)]
pub struct PuppetSelectionGizmoState {
    document: Document,
    selected: Vec<Index>,

    /// All the resize handles this gizmo uses.
    ///
    /// Resize handles are stored counterclockwise from northwest; in the
    /// order NW - N - NE - E - SE - S - SW - W
    resize_handles: Option<[gtk4::Image; 8]>,

    /// Copy of the puppet bounds at time of start of a drag operation.
    ///
    /// Puppets can change shape by deforming, so we need to have a consistent
    /// size to keep the scaling operation numerically stable.
    ///
    /// This stores both the overall bounds as well as each individual puppet
    /// bounds and scale.
    resize_bounds: Option<(
        Option<graphene::Rect>,
        Vec<(Option<graphene::Rect>, glam::Vec2, f32)>,
    )>,
}

#[derive(Default)]
pub struct PuppetSelectionGizmoImp {
    state: Rc<RefCell<PuppetSelectionGizmoState>>,
}

#[glib::object_subclass]
impl ObjectSubclass for PuppetSelectionGizmoImp {
    const NAME: &'static str = "NGTPuppetSelectionGizmo";
    type Type = PuppetSelectionGizmo;
    type ParentType = gtk4::Widget;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-selection");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for PuppetSelectionGizmoImp {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj().set_size_request(0, 0);
    }
}

impl WidgetImpl for PuppetSelectionGizmoImp {
    fn snapshot(&self, snapshot: &gtk4::Snapshot) {
        let state = self.state.borrow_mut();

        if let Some(handles) = state.resize_handles.as_ref() {
            for handle in handles {
                self.obj().snapshot_child(handle, snapshot);
            }
        }
    }
}

impl ScrollableImpl for PuppetSelectionGizmoImp {}

impl PuppetSelectionGizmoImp {
    /// Store the bounds of all selected puppets for later use in operations
    /// that require numerical stability (i.e. we want to be able to scale
    /// puppets without their bounds changing).
    ///
    /// While bounds are stored, puppet updates will be ignored. Call
    /// `clear_bounds` to re-enable puppet updates.
    fn store_bounds(&self) {
        let mut state = self.state.borrow_mut();

        let mut union_bounds = None;
        let mut individual_bounds = vec![];

        for puppet_index in &state.selected {
            if let Some(puppet) = state.document.stage().puppet(*puppet_index) {
                let puppet_bounds = puppet.bounds().map(|inox_bounds| {
                    let tl = inox_bounds.top_left_point();
                    let br = inox_bounds.bottom_right_point();

                    let width = br.x - tl.x;
                    let height = br.y - tl.y;

                    graphene::Rect::new(tl.x, tl.y, width, height)
                        .scale(puppet.scale(), puppet.scale())
                        .offset_r(puppet.position().x, puppet.position().y)
                });

                match (union_bounds, puppet_bounds) {
                    (None, puppet_bounds) => union_bounds = puppet_bounds,
                    (Some(union), Some(puppet_bounds)) => {
                        union_bounds = Some(union.union(&puppet_bounds))
                    }
                    _ => {}
                };

                individual_bounds.push((puppet_bounds, puppet.position(), puppet.scale()));
            } else {
                individual_bounds.push((None, glam::Vec2::ZERO, 1.0));
            }
        }

        state.resize_bounds = Some((union_bounds, individual_bounds));
    }

    /// Scale all of the selected puppets around the given origin point by the
    /// provided scale value. Coordinates are in stage space.
    ///
    /// This uses the resize bounds and will do nothing without them. If they
    /// are stale, the user will see the wrong transform. Call `store_bounds`
    /// first.
    fn rescale(&self, origin: &graphene::Point, scale: f32) {
        let state = self.state.borrow_mut();
        let mut document = state.document.clone();
        let stage = self.obj().closest::<StageWidget>().unwrap();

        if let Some((Some(union), puppet_bounds)) = &state.resize_bounds {
            let union_tl = union.top_left();
            let union_br = union.bottom_right();

            let scaled_union_tl_x = ((union_tl.x() - origin.x()) * scale) + origin.x();
            let scaled_union_tl_y = ((union_tl.y() - origin.y()) * scale) + origin.y();

            let scaled_union_br_x = ((union_br.x() - origin.x()) * scale) + origin.x();
            let scaled_union_br_y = ((union_br.y() - origin.y()) * scale) + origin.y();
            for (index, (bounds, old_pos, old_scale)) in state.selected.iter().zip(puppet_bounds) {
                if let Some(bounds) = bounds {
                    let bounds_tl = bounds.top_left();
                    let bounds_br = bounds.bottom_right();

                    let width = bounds_br.x() - bounds_tl.x();

                    let scaled_origin_x = ((old_pos.x - origin.x()) * scale) + origin.x();
                    let scaled_origin_y = ((old_pos.y - origin.y()) * scale) + origin.y();

                    let scaled_bounds_tl_x = ((bounds_tl.x() - origin.x()) * scale) + origin.x();
                    let scaled_bounds_br_x = ((bounds_br.x() - origin.x()) * scale) + origin.x();

                    //TODO: I'm not sure if this is actually necessary?
                    let scaled_width = scaled_bounds_br_x - scaled_bounds_tl_x;
                    let extra_scale = scaled_width / width;

                    if let Some(mut puppet) = document.stage_mut().puppet(*index) {
                        puppet.set_position(glam::Vec2::new(scaled_origin_x, scaled_origin_y));
                        puppet.set_scale(old_scale * extra_scale);
                    }
                }
            }

            let stage_tl = stage
                .project_stage_to_viewport(glam::Vec2::new(scaled_union_tl_x, scaled_union_tl_y));
            let stage_br = stage
                .project_stage_to_viewport(glam::Vec2::new(scaled_union_br_x, scaled_union_br_y));

            let scaled_union_width = stage_br.x - stage_tl.x;
            let scaled_union_height = stage_br.y - stage_tl.y;

            // During the rescale operation, we don't update our bounds, so we
            // must scale ourselves.
            //
            // This is a preview, when it's all over we'll have the stage
            // recalculate everything.
            self.obj()
                .measure(gtk4::Orientation::Horizontal, scaled_union_width as i32);
            self.obj().allocate(
                scaled_union_width as i32,
                scaled_union_height as i32,
                -1,
                Some(
                    gsk4::Transform::new().translate(&graphene::Point::new(stage_tl.x, stage_tl.y)),
                ),
            );
        }
    }

    /// Clear any previously stored puppet bounds.
    fn clear_bounds(&self) {
        self.state.borrow_mut().resize_bounds = None;
    }

    /// Create a handle resize gizmo that can be dragged to rescale the
    /// current selection.
    ///
    /// See `init_resize_handles` for an explanation of the resize handle
    /// system at large, including the meaning of the indexes.
    fn create_resize_handle(
        &self,
        index: usize,
        cursor: &str,
        handle_graphic: &str,
    ) -> gtk4::Image {
        let handle_widget = gtk4::Image::builder()
            .resource(handle_graphic)
            .cursor(&gdk4::Cursor::from_name(cursor, None).unwrap())
            .build();
        handle_widget.set_parent(&*self.obj());

        let handle_drag = gtk4::GestureDrag::builder()
            .button(gdk4::BUTTON_PRIMARY)
            .build();
        let handle_drag_begin_self = self.obj().clone().downgrade();
        handle_drag.connect_drag_begin(move |drag, _, _| {
            drag.set_state(gtk4::EventSequenceState::Claimed);

            if let Some(me) = handle_drag_begin_self.upgrade() {
                me.imp().store_bounds();
            }
        });

        let handle_drag_update_self = self.obj().clone().downgrade();
        let handle_drag_update_handle = handle_widget.downgrade();
        handle_drag.connect_drag_update(move |_, x, y| {
            if let (Some(me), Some(handle_widget)) = (
                handle_drag_update_self.upgrade(),
                handle_drag_update_handle.upgrade(),
            ) {
                let state = me.imp().state.borrow();

                let stage = me.closest::<StageWidget>().unwrap();
                let mouse = handle_widget
                    .compute_point(&stage, &graphene::Point::new(x as f32, y as f32))
                    .unwrap();
                let stage_mouse =
                    stage.project_viewport_to_stage(glam::Vec2::new(mouse.x(), mouse.y()));

                if let Some((Some(union_bounds), _)) = &state.resize_bounds {
                    let union_bounds_tl = union_bounds.top_left();
                    let union_bounds_br = union_bounds.bottom_right();

                    // We want to drag from the opposing point - i.e. W side's
                    // origin is E!
                    let origin_x = match index {
                        0 | 6..8 => union_bounds_br.x(),
                        1 | 5 => (union_bounds_br.x() + union_bounds_tl.x()) / 2.0,
                        2..5 => union_bounds_tl.x(),
                        8.. => unreachable!(),
                    };
                    let origin_y = match index {
                        0..3 => union_bounds_br.y(),
                        3 | 7 => (union_bounds_br.y() + union_bounds_tl.y()) / 2.0,
                        4..7 => union_bounds_tl.y(),
                        8.. => unreachable!(),
                    };

                    let my_x = match index {
                        0 | 6..8 => union_bounds_tl.x(),
                        1 | 5 => (union_bounds_br.x() + union_bounds_tl.x()) / 2.0,
                        2..5 => union_bounds_br.x(),
                        8.. => unreachable!(),
                    };
                    let my_y = match index {
                        0..3 => union_bounds_tl.y(),
                        3 | 7 => (union_bounds_br.y() + union_bounds_tl.y()) / 2.0,
                        4..7 => union_bounds_br.y(),
                        8.. => unreachable!(),
                    };

                    let origin_point = glam::Vec2::new(origin_x, origin_y);
                    let my_point = glam::Vec2::new(my_x, my_y);

                    let origin_my_distance = origin_point.distance(my_point);
                    let mouse_distance = origin_point.distance(stage_mouse);

                    let scale = mouse_distance / origin_my_distance;

                    drop(state);
                    me.imp()
                        .rescale(&graphene::Point::new(origin_x, origin_y), scale);
                }
            }
        });

        let handle_drag_end_self = self.obj().clone().downgrade();
        handle_drag.connect_drag_end(move |_, _, _| {
            if let Some(me) = handle_drag_end_self.upgrade() {
                // We don't need to do anything else to "commit" the resize,
                // since we update the puppets normally every frame, though we
                // technically could support rollback.
                me.imp().clear_bounds();
            }
        });

        handle_widget.add_controller(handle_drag);

        handle_widget
    }

    /// Create all the resize handles this gizmo needs.
    ///
    /// Resize handles are numbered 0 to 7, in clockwise order from the NW
    /// corner of the block. e.g. NW, N, NE, E, SE, S, SW, W
    fn init_resize_handles(&self) {
        let mut state = self.state.borrow_mut();
        if state.resize_handles.is_none() {
            let handles = [
                (
                    "nw-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ne-sw-resize.svg",
                ), //Northeast
                (
                    "n-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ns-resize.svg",
                ), //North
                (
                    "ne-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/nw-se-resize.svg",
                ), //Northwest
                (
                    "e-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ew-resize.svg",
                ), //West
                (
                    "se-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ne-sw-resize.svg",
                ), //Southwest
                (
                    "s-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ns-resize.svg",
                ), //South
                (
                    "sw-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/nw-se-resize.svg",
                ), //Southeast
                (
                    "w-resize",
                    "/live/arcturus/ningyotsukai/stage/gizmos/selection/ew-resize.svg",
                ), //East
            ];

            let mut handle_widgets = Vec::with_capacity(8);

            for (index, (cursor, handle_graphic)) in handles.into_iter().enumerate() {
                handle_widgets.push(self.create_resize_handle(index, cursor, handle_graphic));
            }

            state.resize_handles = Some(handle_widgets[0..8].as_array().unwrap().clone());
        }
    }

    fn place_resize_handles(&self) {
        self.init_resize_handles();

        let state = self.state.borrow();
        for (index, handle) in state.resize_handles.as_ref().unwrap().iter().enumerate() {
            let (width_minimum, _, _, _) = handle.measure(gtk4::Orientation::Horizontal, 32);
            let (height_minimum, _, _, _) = handle.measure(gtk4::Orientation::Vertical, 32);

            let mut x = 0.0;
            let mut y = 0.0;

            match index {
                0..3 => y -= height_minimum as f32,
                3 | 7 => y += self.obj().height() as f32 / 2.0 - height_minimum as f32 / 2.0,
                4..7 => y += self.obj().height() as f32,
                8.. => unreachable!(),
            }

            match index {
                0 | 6..8 => x -= width_minimum as f32,
                1 | 5 => x += self.obj().width() as f32 / 2.0 - width_minimum as f32 / 2.0,
                2..5 => x += self.obj().width() as f32,
                8.. => unreachable!(),
            }

            handle.allocate(
                width_minimum,
                height_minimum,
                -1,
                Some(gsk4::Transform::new().translate(&graphene::Point::new(x, y))),
            );
            handle.set_visible(true);
        }
    }
}

glib::wrapper! {
    pub struct PuppetSelectionGizmo(ObjectSubclass<PuppetSelectionGizmoImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl PuppetSelectionGizmo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_document(&self, document: Document) {
        let mut state = self.imp().state.borrow_mut();

        state.resize_bounds = None;
        state.selected = vec![];
        state.document = document;
    }

    /// Called by the stage to inform this gesture that the selection has
    /// changed.
    pub fn selection_changed<'a>(
        &self,
        stage: &StageWidget,
        selection: impl Iterator<Item = &'a Index>,
        gizmos: &HashMap<Index, impl IsA<gtk4::Widget>>,
    ) {
        // Bogus measurement
        self.measure(gtk4::Orientation::Horizontal, 10);

        let mut new_bounds = None;
        let mut selected = vec![];

        for index in selection {
            let gizmo = gizmos.get(index);
            if let Some(gizmo) = gizmo {
                let gizmo_bounds = gizmo.compute_bounds(stage);

                match (new_bounds, gizmo_bounds) {
                    (None, gizmo_bounds) => new_bounds = gizmo_bounds,
                    (Some(nb), Some(gizmo_bounds)) => new_bounds = Some(nb.union(&gizmo_bounds)),
                    _ => {}
                };

                selected.push(*index);
            }
        }

        let mut state = self.imp().state.borrow_mut();

        // If we have stored bounds, don't allow the selection to change!
        // Let the user finish dragging first!
        if state.resize_bounds.is_some() {
            //return;
        }

        state.selected = selected;

        drop(state);

        if let Some(bounds) = new_bounds {
            self.set_visible(true);
            self.allocate(
                bounds.width() as i32,
                bounds.height() as i32,
                -1,
                Some(gsk4::Transform::new().translate(&bounds.top_left())),
            );
            self.imp().place_resize_handles();
        } else {
            self.set_visible(false);
        }
    }
}
