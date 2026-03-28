use gtk4::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

use crate::stage::StageWidget;

fn rect_from_points(start: [f64; 2], end: [f64; 2]) -> gdk4::Rectangle {
    let mut x = start[0] as i32;
    let mut y = start[1] as i32;
    let mut width = end[0] as i32 - start[0] as i32;
    let mut height = end[1] as i32 - start[1] as i32;

    if width < 0 {
        x = end[0] as i32;
        width *= -1;
    }

    if height < 0 {
        y = end[1] as i32;
        height *= -1;
    }

    gdk4::Rectangle::new(x, y, width, height)
}

/// Constructs and manages a select gesture that allows selecting one or more
/// items on the stage by clicking and dragging.
#[derive(Clone)]
pub struct SelectGesture(Rc<RefCell<SelectGestureImp>>);
pub struct SelectGestureImp {
    starting_position: Option<[f64; 2]>,
    ending_position: Option<[f64; 2]>,
    gizmo: gtk4::Widget,
}

impl SelectGesture {
    pub fn for_widget(widget: &StageWidget, gizmo: &impl IsA<gtk4::Widget>) -> Self {
        let selfish = SelectGesture(Rc::new(RefCell::new(SelectGestureImp {
            starting_position: None,
            ending_position: None,
            gizmo: gizmo.clone().upcast(),
        })));

        let select = gtk4::GestureDrag::builder()
            .button(gdk4::BUTTON_PRIMARY)
            .build();

        let begin_self = selfish.clone();
        select.connect_drag_begin(move |_, x, y| {
            let mut state = begin_self.0.borrow_mut();

            state.starting_position = Some([x, y]);
        });

        let update_self = selfish.clone();
        select.connect_drag_update(move |_, x, y| {
            let mut state = update_self.0.borrow_mut();

            let start = state.starting_position.unwrap();

            state.ending_position = Some([x + start[0], y + start[1]]);

            let rect = rect_from_points(start, state.ending_position.unwrap());

            if rect.width() > 2 && rect.height() > 2 {
                state.gizmo.measure(gtk4::Orientation::Horizontal, 4);
                state.gizmo.allocate(
                    rect.width() as i32,
                    rect.height() as i32,
                    -1,
                    Some(
                        gsk4::Transform::new()
                            .translate(&graphene::Point::new(rect.x() as f32, rect.y() as f32)),
                    ),
                );
                state.gizmo.set_visible(true);
            } else {
                state.gizmo.set_visible(false);
            }
        });

        let begin_self = selfish.clone();
        select.connect_drag_end({
            let weak_widget = widget.downgrade();
            move |_, _x, _y| {
                let state = begin_self.0.borrow_mut();

                if let Some(widget) = weak_widget.upgrade() {
                    if let Some(bounds) = state.gizmo.compute_bounds(&widget) {
                        widget.set_selected_to_area(bounds);
                    }
                }

                state.gizmo.set_visible(false);
            }
        });

        widget.add_controller(select);

        selfish
    }
}
