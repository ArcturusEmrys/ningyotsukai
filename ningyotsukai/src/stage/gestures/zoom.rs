use gtk4::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

/// Manages a gesture that allows zooming into or out of the stage by either
/// scrolling with the mouse wheel while the middle button is held, or by
/// zooming with a touch surface.
#[derive(Clone)]
pub struct ZoomGesture(Rc<RefCell<ZoomGestureImp>>);
pub struct ZoomGestureImp {
    zadjustment: Option<gtk4::Adjustment>,

    /// Whether or not the middle mouse button is down.
    middle_mouse_button_down: bool,

    /// The starting zoom factor at the start of GestureZoom's begin signal
    starting_zoom_amount: f64,
}

impl ZoomGesture {
    /// Create a gesture that allows zooming into or out of the stage by
    /// either scrolling with the mouse wheel while the middle button is held,
    /// or by zooming with a touch surface.
    pub fn for_widget(widget: &gtk4::Widget) -> Self {
        let selfish = ZoomGesture(Rc::new(RefCell::new(ZoomGestureImp {
            zadjustment: None,
            middle_mouse_button_down: false,
            starting_zoom_amount: 0.0,
        })));

        let middle_click = gtk4::GestureClick::builder()
            .button(gdk4::BUTTON_MIDDLE)
            .build();
        let middle_click_pressed_self = selfish.clone();
        middle_click.connect_pressed(move |_, _, _, _| {
            middle_click_pressed_self
                .0
                .borrow_mut()
                .middle_mouse_button_down = true;
        });

        let middle_click_released_self = selfish.clone();
        middle_click.connect_released(move |_, _, _, _| {
            middle_click_released_self
                .0
                .borrow_mut()
                .middle_mouse_button_down = false;
        });

        widget.add_controller(middle_click);

        let scroll_wheel = gtk4::EventControllerScroll::builder()
            .flags(gtk4::EventControllerScrollFlags::VERTICAL)
            .build();

        let scroll_wheel_self = selfish.clone();
        scroll_wheel.connect_scroll(move |_, _, dy| {
            let state = scroll_wheel_self.0.borrow();
            let mmb_down = state.middle_mouse_button_down;
            if mmb_down {
                // With a normal mouse, dy yields either 1 or -1.
                if let Some(ref z_adjust) = state.zadjustment {
                    z_adjust.set_value(z_adjust.value() + z_adjust.step_increment() * dy * -1.0);
                }

                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        widget.add_controller(scroll_wheel);

        let zoom = gtk4::GestureZoom::new();

        let zoom_begin_self = selfish.clone();
        zoom.connect_begin(move |_, _| {
            let mut state = zoom_begin_self.0.borrow_mut();
            if let Some(ref zadjust) = state.zadjustment {
                state.starting_zoom_amount = zadjust.value();
            }
        });

        let zoom_scale_changed_self = selfish.clone();
        zoom.connect_scale_changed(move |_, delta| {
            let state = zoom_scale_changed_self.0.borrow();
            //TODO: I have yet to actually test this code on a real trackpad or touchscreen yet
            if let Some(ref zadjust) = state.zadjustment {
                //I'm assuming GTK provides linear zoom values as multiples (not percentages)
                zadjust.set_value(state.starting_zoom_amount + delta.log(10.0));
            }
        });

        widget.add_controller(zoom);

        selfish
    }

    pub fn set_zadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        self.0.borrow_mut().zadjustment = adjust;
    }
}
