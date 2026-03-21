use glib;
use graphene;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::stage::border::StageBorderGizmo;

#[derive(Default)]
pub struct StageWidgetState {
    document: Arc<Mutex<Document>>,

    /// Internal accounting widget for the border around the stage.
    border_gizmo: Option<StageBorderGizmo>,

    /// The scroll position at the time our middle-click drag recognizer
    /// started.
    starting_drag_position: Option<[i32; 2]>,

    /// Whether or not the middle mouse button is down.
    middle_mouse_button_down: bool,
}

#[derive(glib::Properties)]
#[properties(wrapper_type=StageWidget)]
pub struct StageWidgetImp {
    state: RefCell<StageWidgetState>,

    //The derive macros MANDATE a storage location for properties, even if you
    //plan to fully synthesize them
    #[property(get, set=Self::set_hadjustment, override_interface=gtk4::Scrollable)]
    hadjustment: RefCell<Option<gtk4::Adjustment>>,

    #[property(get, set=Self::set_vadjustment, override_interface=gtk4::Scrollable)]
    vadjustment: RefCell<Option<gtk4::Adjustment>>,

    #[property(name="hscroll-policy", get, set, override_interface=gtk4::Scrollable)]
    hscroll_policy: Cell<gtk4::ScrollablePolicy>,

    #[property(name="vscroll-policy", get, set, override_interface=gtk4::Scrollable)]
    vscroll_policy: Cell<gtk4::ScrollablePolicy>,

    //Our zoom adjustment. Might be wired up to a widget at some point.
    #[property(get, set=Self::set_zadjustment)]
    zadjustment: RefCell<Option<gtk4::Adjustment>>,
}

impl Default for StageWidgetImp {
    fn default() -> Self {
        Self {
            state: RefCell::new(StageWidgetState::default()),
            hadjustment: RefCell::new(None),
            vadjustment: RefCell::new(None),
            hscroll_policy: Cell::new(gtk4::ScrollablePolicy::Natural),
            vscroll_policy: Cell::new(gtk4::ScrollablePolicy::Natural),
            zadjustment: RefCell::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for StageWidgetImp {
    const NAME: &'static str = "NGTStageWidget";
    type Type = StageWidget;
    type ParentType = gtk4::Widget;
    type Interfaces = (gtk4::Scrollable,);

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-stage");
    }

    fn instance_init(obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageWidgetImp {
    fn constructed(&self) {
        self.parent_constructed();

        let border_gizmo = StageBorderGizmo::new();

        border_gizmo.set_parent(&*self.obj());
        self.state.borrow_mut().border_gizmo = Some(border_gizmo);

        let drag = gtk4::GestureDrag::builder()
            .button(gdk4::BUTTON_MIDDLE)
            .build();

        let drag_begin_self = self.obj().clone();
        drag.connect_drag_begin(move |_, _, _| {
            let mut state = drag_begin_self.imp().state.borrow_mut();

            if let (Some(h_adjust), Some(v_adjust)) = (
                &*drag_begin_self.imp().hadjustment.borrow(),
                &*drag_begin_self.imp().vadjustment.borrow(),
            ) {
                state.starting_drag_position =
                    Some([h_adjust.value() as i32, v_adjust.value() as i32]);
            }
        });

        let drag_drag_self = self.obj().clone();
        drag.connect_drag_update(move |_, offset_x, offset_y| {
            let state = drag_drag_self.imp().state.borrow();

            if let (Some(starting_drag_position), Some(h_adjust), Some(v_adjust)) = (
                state.starting_drag_position,
                &*drag_drag_self.imp().hadjustment.borrow(),
                &*drag_drag_self.imp().vadjustment.borrow(),
            ) {
                h_adjust.set_value(starting_drag_position[0] as f64 - offset_x);
                v_adjust.set_value(starting_drag_position[1] as f64 - offset_y);
            }
        });

        self.obj().add_controller(drag);

        let middle_click = gtk4::GestureClick::builder()
            .button(gdk4::BUTTON_MIDDLE)
            .build();
        let middle_click_pressed_self = self.obj().clone();
        middle_click.connect_pressed(move |_, _, _, _| {
            middle_click_pressed_self
                .imp()
                .state
                .borrow_mut()
                .middle_mouse_button_down = true;
        });

        let middle_click_released_self = self.obj().clone();
        middle_click.connect_released(move |_, _, _, _| {
            middle_click_released_self
                .imp()
                .state
                .borrow_mut()
                .middle_mouse_button_down = false;
        });

        self.obj().add_controller(middle_click);

        let scroll_wheel = gtk4::EventControllerScroll::builder()
            .flags(gtk4::EventControllerScrollFlags::VERTICAL)
            .build();

        let scroll_wheel_self = self.obj().clone();
        scroll_wheel.connect_scroll(move |_, _, dy| {
            let mmb_down = scroll_wheel_self
                .imp()
                .state
                .borrow()
                .middle_mouse_button_down;
            if mmb_down {
                // With a normal mouse, dy yields either 1 or -1.
                if let Some(ref z_adjust) = *scroll_wheel_self.imp().zadjustment.borrow() {
                    z_adjust.set_value(z_adjust.value() + z_adjust.step_increment() * dy * -1.0);
                }

                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        self.obj().add_controller(scroll_wheel);
    }

    fn dispose(&self) {
        if let Some(gizmo) = self.state.borrow_mut().border_gizmo.take() {
            gizmo.unparent();
        }
    }
}

impl WidgetImpl for StageWidgetImp {
    fn snapshot(&self, snapshot: &gtk4::Snapshot) {
        let state = self.state.borrow();
        let document = state.document.lock().unwrap();

        snapshot.push_clip(&graphene::Rect::new(
            0.0,
            0.0,
            self.obj().width() as f32,
            self.obj().height() as f32,
        ));

        let size = document.stage().size();

        // This is a log scale, but we need linear zoom.
        // See document/controller.ui for more info
        let zoom = 10.0_f32.powf(
            self.zadjustment
                .borrow()
                .as_ref()
                .map(|z| z.value())
                .unwrap_or(1.0) as f32,
        );

        snapshot.scale(zoom, zoom);

        let hscroll_offset = self
            .hadjustment
            .borrow()
            .as_ref()
            .map(|v| v.value())
            .unwrap_or(0.0) as f32;
        let vscroll_offset = self
            .vadjustment
            .borrow()
            .as_ref()
            .map(|v| v.value())
            .unwrap_or(0.0) as f32;

        snapshot.translate(&graphene::Point::new(-hscroll_offset, -vscroll_offset));

        snapshot.append_color(
            &gdk4::RGBA::new(1.0, 1.0, 1.0, 1.0),
            &graphene::Rect::new(0.0, 0.0, size.x(), size.y()),
        );

        if let Some(ref border) = self.state.borrow().border_gizmo {
            self.obj().snapshot_child(border, snapshot);
        }

        snapshot.pop();
    }

    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);

        self.configure_adjustments();
    }
}

impl ScrollableImpl for StageWidgetImp {}

impl StageWidgetImp {
    /// Configure all the GTK properties to match the current state of the
    /// stage. Must be called whenever:
    ///
    /// 1. The contents of the stage change
    /// 2. Adjustments are set
    /// 3. The window is resized
    fn configure_adjustments(&self) {
        let state = self.state.borrow();
        let document = state.document.lock().unwrap();

        let width = self.obj().width();
        let height = self.obj().height();

        let stage_width = document.stage().size().x();
        let stage_height = document.stage().size().y();

        //TODO: Off-stage scrolling should be limited to:
        // 1. Minimum: 3/4ths the window size (so you can't normally scroll the stage off)
        // 2. The furthest stage object in that direction (so you can get at things you accidentally put there)
        if let Some(ref adjust) = *self.hadjustment.borrow() {
            adjust.set_lower((stage_width * -1.0) as f64);
            adjust.set_upper((stage_width * 2.0) as f64);
            adjust.set_page_increment(width as f64);
            adjust.set_page_size(width as f64);
        }

        if let Some(ref adjust) = *self.vadjustment.borrow() {
            adjust.set_lower((stage_height * -1.0) as f64);
            adjust.set_upper((stage_height * 2.0) as f64);
            adjust.set_page_increment(height as f64);
            adjust.set_page_size(height as f64);
        }

        if let Some(ref border) = self.state.borrow().border_gizmo {
            // If we don't measure our children, GTK complains
            border.measure(gtk4::Orientation::Horizontal, stage_width as i32);
            border.allocate(stage_width as i32, stage_height as i32, -1, None);
        }
    }

    fn set_hadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.hadjustment.borrow_mut() = adjust;

        self.configure_adjustments();
    }

    fn set_vadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.vadjustment.borrow_mut() = adjust;

        self.configure_adjustments();
    }

    fn set_zadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.zadjustment.borrow_mut() = adjust;

        self.configure_adjustments();
    }
}

glib::wrapper! {
    pub struct StageWidget(ObjectSubclass<StageWidgetImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Scrollable;
}

impl StageWidget {
    pub fn new() -> Self {
        let selfish: StageWidget = glib::Object::builder().build();

        selfish.bind();

        selfish
    }

    pub fn set_document(&self, document: Arc<Mutex<Document>>) {
        self.imp().state.borrow_mut().document = document;
        self.imp().configure_adjustments();
    }

    fn bind(&self) {
        self.set_hexpand(true);
        self.set_vexpand(true);
    }
}
