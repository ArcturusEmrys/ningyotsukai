use gio;
use glib;
use glib::object::ObjectExt;
use graphene;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::{AdjustmentExt, ScrollableExt, SnapshotExt, SnapshotExtManual, WidgetExt};
use gtk4::subclass::prelude::*;

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use crate::document::Document;

#[derive(Default)]
pub struct StageWidgetState {
    document: Arc<Mutex<Document>>,
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
}

impl Default for StageWidgetImp {
    fn default() -> Self {
        Self {
            state: RefCell::new(StageWidgetState::default()),
            hadjustment: RefCell::new(None),
            vadjustment: RefCell::new(None),
            hscroll_policy: Cell::new(gtk4::ScrollablePolicy::Natural),
            vscroll_policy: Cell::new(gtk4::ScrollablePolicy::Natural),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for StageWidgetImp {
    const NAME: &'static str = "NGTStageWidget";
    type Type = StageWidget;
    type ParentType = gtk4::Widget;
    type Interfaces = (gtk4::Scrollable,);

    fn class_init(class: &mut Self::Class) {}

    fn instance_init(obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageWidgetImp {
    fn constructed(&self) {
        self.parent_constructed();
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

        // TODO: Add a border and make it CSS styleable
        //snapshot.append_border(outline, border_width, border_color);

        snapshot.pop();
    }

    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);

        self.configure_adjustments();
    }
}

impl ScrollableImpl for StageWidgetImp {}

impl StageWidgetImp {
    fn configure_adjustments(&self) {
        let state = self.state.borrow();
        let document = state.document.lock().unwrap();

        //TODO: Off-stage scrolling should be limited to:
        // 1. Minimum: 3/4ths the window size (so you can't normally scroll the stage off)
        // 2. The furthest stage object in that direction (so you can get at things you accidentally put there)
        if let Some(ref adjust) = *self.hadjustment.borrow() {
            adjust.set_lower((document.stage().size().x() * -1.0) as f64);
            adjust.set_upper((document.stage().size().x() * 2.0) as f64);
            adjust.set_page_increment(self.obj().width() as f64);
            adjust.set_page_size(self.obj().width() as f64);
        }

        if let Some(ref adjust) = *self.vadjustment.borrow() {
            adjust.set_lower((document.stage().size().y() * -1.0) as f64);
            adjust.set_upper((document.stage().size().y() * 2.0) as f64);
            adjust.set_page_increment(self.obj().height() as f64);
            adjust.set_page_size(self.obj().height() as f64);
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
