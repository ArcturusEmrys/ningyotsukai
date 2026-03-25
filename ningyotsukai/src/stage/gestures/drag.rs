use gtk4::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

/// Constructs and manages a drag gesture that allows panning across the stage
/// by dragging with the middle mouse button.
#[derive(Clone)]
pub struct DragGesture(Rc<RefCell<DragGestureImp>>);
pub struct DragGestureImp {
    hadjustment: Option<gtk4::Adjustment>,
    vadjustment: Option<gtk4::Adjustment>,
    zadjustment: Option<gtk4::Adjustment>,

    /// The scroll position at the time our middle-click drag recognizer
    /// started.
    starting_drag_position: Option<[i32; 2]>,
}

impl DragGesture {
    /// Create a drag controller gesture that allows panning across the stage
    /// by dragging with the middle mouse button.
    pub fn for_widget(widget: &gtk4::Widget) -> Self {
        let selfish = DragGesture(Rc::new(RefCell::new(DragGestureImp {
            hadjustment: None,
            vadjustment: None,
            zadjustment: None,
            starting_drag_position: None,
        })));

        let drag = gtk4::GestureDrag::builder()
            .button(gdk4::BUTTON_MIDDLE)
            .build();

        let drag_begin_self = selfish.clone();
        drag.connect_drag_begin(move |_, _, _| {
            let mut state = drag_begin_self.0.borrow_mut();

            if let (Some(h_adjust), Some(v_adjust)) = (&state.hadjustment, &state.vadjustment) {
                state.starting_drag_position =
                    Some([h_adjust.value() as i32, v_adjust.value() as i32]);
            }
        });

        let drag_drag_self = selfish.clone();
        drag.connect_drag_update(move |_, mut offset_x, mut offset_y| {
            let state = drag_drag_self.0.borrow_mut();

            if let (Some(starting_drag_position), Some(h_adjust), Some(v_adjust), Some(z_adjust)) = (
                &state.starting_drag_position,
                &state.hadjustment,
                &state.vadjustment,
                &state.zadjustment,
            ) {
                // When we zoom out, or in, our drag speed changes, so adjust for that.
                let zoom = 10.0_f64.powf(z_adjust.value());
                offset_x /= zoom;
                offset_y /= zoom;

                h_adjust.set_value(starting_drag_position[0] as f64 - offset_x);
                v_adjust.set_value(starting_drag_position[1] as f64 - offset_y);
            }
        });

        widget.add_controller(drag);

        selfish
    }

    pub fn set_hadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        self.0.borrow_mut().hadjustment = adjust;
    }

    pub fn set_vadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        self.0.borrow_mut().vadjustment = adjust;
    }

    pub fn set_zadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        self.0.borrow_mut().zadjustment = adjust;
    }
}
