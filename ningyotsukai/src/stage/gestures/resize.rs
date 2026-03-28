use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use generational_arena::Index;

use crate::document::Document;
use crate::stage::StageWidget;
use gtk4::prelude::*;

pub struct ResizeGestures(Rc<RefCell<ResizeGesturesImp>>);
pub struct ResizeGesturesImp {
    my_bounds: Option<graphene::Rect>,
    document: Arc<Mutex<Document>>,
    selected: Vec<Index>,
    resize_handles: Option<[gtk4::Image; 8]>,
}

impl ResizeGestures {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        ResizeGestures(Rc::new(RefCell::new(ResizeGesturesImp {
            my_bounds: None,
            document,
            selected: vec![],
            resize_handles: None,
        })))
    }

    pub fn set_document(&self, document: Arc<Mutex<Document>>) {
        let mut state = self.0.borrow_mut();
        state.my_bounds = None;
        state.selected = vec![];
        state.document = document;
    }

    /// Called by the stage to inform this gesture that the selection has
    /// changed.
    pub fn selection_changed<'a>(
        &self,
        stage: &StageWidget,
        selection: impl Iterator<Item = (Index, &'a impl IsA<gtk4::Widget>)>,
    ) {
        let mut new_bounds = None;
        let mut selected = vec![];

        for (index, gizmo) in selection {
            let gizmo_bounds = gizmo.compute_bounds(stage);

            match (new_bounds, gizmo_bounds) {
                (None, gizmo_bounds) => new_bounds = gizmo_bounds,
                (Some(nb), Some(gizmo_bounds)) => new_bounds = Some(nb.union(&gizmo_bounds)),
                _ => {}
            };

            selected.push(index);
        }

        let mut state = self.0.borrow_mut();

        state.my_bounds = new_bounds;
        state.selected = selected;
    }

    pub fn place_resize_handles(&self, stage: &StageWidget) {
        let mut state = self.0.borrow_mut();
        if state.resize_handles.is_none() {
            let handles = vec![
                gtk4::Image::builder().icon_name("ne-resize").build(),
                gtk4::Image::builder().icon_name("n-resize").build(),
                gtk4::Image::builder().icon_name("nw-resize").build(),
                gtk4::Image::builder().icon_name("w-resize").build(),
                gtk4::Image::builder().icon_name("sw-resize").build(),
                gtk4::Image::builder().icon_name("s-resize").build(),
                gtk4::Image::builder().icon_name("se-resize").build(),
                gtk4::Image::builder().icon_name("e-resize").build(),
            ];

            for handle in handles.iter() {
                handle.set_parent(stage);

                let (width_minimum, _, _, _) = handle.measure(gtk4::Orientation::Horizontal, 32);
                let (height_minimum, _, _, _) = handle.measure(gtk4::Orientation::Vertical, 32);

                if let Some(bounds) = state.my_bounds {
                    let x = bounds.x();
                    let y = bounds.y();

                    handle.allocate(
                        width_minimum,
                        height_minimum,
                        -1,
                        Some(gsk4::Transform::new().translate(&graphene::Point::new(x, y))),
                    );
                }
            }

            state.resize_handles = Some(handles[0..7].as_array().unwrap().clone());
        }
    }
}
