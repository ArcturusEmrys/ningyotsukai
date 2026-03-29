//! A Gizmo-style class that manages the bounds of an active stage selection.
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use generational_arena::Index;

use crate::document::Document;
use crate::stage::StageWidget;

#[derive(Default)]
pub struct PuppetSelectionGizmoState {
    my_bounds: Option<graphene::Rect>,
    document: Arc<Mutex<Document>>,
    selected: Vec<Index>,
    resize_handles: Option<[gtk4::Image; 8]>,
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
    fn place_resize_handles(&self) {
        let mut state = self.state.borrow_mut();
        if state.resize_handles.is_none() {
            let handles = [
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ne-sw-resize.svg", //Northeast
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ns-resize.svg",    //North
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/nw-se-resize.svg", //Northwest
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ew-resize.svg",    //West
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ne-sw-resize.svg", //Southwest
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ns-resize.svg",    //South
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/nw-se-resize.svg", //Southeast
                "/live/arcturus/ningyotsukai/stage/gizmos/selection/ew-resize.svg",    //East
            ];

            let mut handle_widgets = Vec::with_capacity(8);

            for handle in handles {
                let handle_widget = gtk4::Image::builder().resource(handle).build();
                handle_widget.set_parent(&*self.obj());

                handle_widgets.push(handle_widget);
            }

            state.resize_handles = Some(handle_widgets[0..8].as_array().unwrap().clone());
        }

        for (index, handle) in state.resize_handles.as_ref().unwrap().iter().enumerate() {
            let (width_minimum, _, _, _) = handle.measure(gtk4::Orientation::Horizontal, 32);
            let (height_minimum, _, _, _) = handle.measure(gtk4::Orientation::Vertical, 32);

            if let Some(bounds) = state.my_bounds {
                let mut x = 0.0;
                let mut y = 0.0;

                match index {
                    0..3 => y -= height_minimum as f32,
                    3 | 7 => y += bounds.height() / 2.0 - height_minimum as f32 / 2.0,
                    4..7 => y += bounds.height(),
                    8.. => unreachable!(),
                }

                match index {
                    0 | 6..8 => x -= width_minimum as f32,
                    1 | 5 => x += bounds.width() / 2.0 - width_minimum as f32 / 2.0,
                    2..5 => x += bounds.width(),
                    8.. => unreachable!(),
                }

                handle.allocate(
                    width_minimum,
                    height_minimum,
                    -1,
                    Some(gsk4::Transform::new().translate(&graphene::Point::new(x, y))),
                );
                handle.set_visible(true);
            } else {
                handle.set_visible(false);
            }
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

    pub fn set_document(&self, document: Arc<Mutex<Document>>) {
        let mut state = self.imp().state.borrow_mut();

        state.my_bounds = None;
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

        state.my_bounds = new_bounds;
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
