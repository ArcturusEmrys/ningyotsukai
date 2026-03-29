use glib;
use graphene;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use generational_arena::Index;

use glam::Vec2;

use crate::document::Document;
use crate::stage::gestures::{DragGesture, SelectGesture, ZoomGesture};
use crate::stage::gizmos::{
    DragSelectGizmo, PuppetBoundsGizmo, PuppetSelectionGizmo, StageBorderGizmo,
};
use crate::stage::renderer::StageRenderer;

#[derive(Default)]
pub struct StageWidgetState {
    document: Arc<Mutex<Document>>,

    /// List of puppets that are currently selected.
    selected: HashSet<Index>,

    /// Internal accounting widget for the border around the stage.
    border_gizmo: Option<StageBorderGizmo>,

    /// A gizmo to represent the selection box.
    drag_sel_gizmo: Option<DragSelectGizmo>,

    /// A gizmo for each puppet on the stage.
    puppet_gizmos: HashMap<Index, PuppetBoundsGizmo>,

    /// Rendering area for Inox2D.
    render_area: Option<StageRenderer>,

    /// Our drag gesture.
    drag_gesture: Option<DragGesture>,

    /// Our drag gesture.
    zoom_gesture: Option<ZoomGesture>,

    /// And our select gesture.
    select_gesture: Option<SelectGesture>,

    /// Gizmo for the current selection.
    selection_gizmo: Option<PuppetSelectionGizmo>,

    /// The last tick this widget processed, used to calculate timestamps to
    /// feed to Inox2D.
    last_mus: Option<i64>,
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

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageWidgetImp {
    fn constructed(&self) {
        self.parent_constructed();

        let border_gizmo = StageBorderGizmo::new();

        border_gizmo.set_parent(&*self.obj());
        self.state.borrow_mut().border_gizmo = Some(border_gizmo);

        let drag_sel_gizmo = DragSelectGizmo::new();

        drag_sel_gizmo.set_parent(&*self.obj());
        self.state.borrow_mut().drag_sel_gizmo = Some(drag_sel_gizmo.clone());

        let gl_area = StageRenderer::new();

        self.obj()
            .bind_property("hadjustment", &gl_area, "hadjustment")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        self.obj()
            .bind_property("vadjustment", &gl_area, "vadjustment")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        self.obj()
            .bind_property("zadjustment", &gl_area, "zadjustment")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        gl_area.set_parent(&*self.obj());
        self.state.borrow_mut().render_area = Some(gl_area);
        self.state.borrow_mut().drag_gesture =
            Some(DragGesture::for_widget(&self.obj().clone().upcast()));
        self.state.borrow_mut().zoom_gesture =
            Some(ZoomGesture::for_widget(&self.obj().clone().upcast()));
        self.state.borrow_mut().select_gesture =
            Some(SelectGesture::for_widget(&*self.obj(), &drag_sel_gizmo));

        let tick = RefCell::new(Some(self.obj().add_tick_callback(move |me, clock| {
            let mut state = me.imp().state.borrow_mut();

            let mus = clock.frame_time();
            let Some(last_mus) = state.last_mus else {
                state.last_mus = Some(mus);
                return glib::ControlFlow::Continue;
            };

            state.last_mus = Some(mus);
            drop(state);

            let del_mus = mus - last_mus;
            let dt = del_mus as f32 / 1_000_000.0;

            me.imp().update_puppets(dt);

            glib::ControlFlow::Continue
        })));

        self.obj().connect_unrealize(move |_| {
            // Rust's type system doesn't support destructors, because it's
            // missing some kind of "owned reference" type, so Drop and
            // destroy require you to pretend the object needs to still be
            // logically valid just in case someone... grabs it out of the
            // trash, somehow?
            //
            // Hence we have to store an option, just so we can .take() the
            // callback and remove it.
            tick.borrow_mut().take().map(|tick| tick.remove());
        });
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

        snapshot.save();

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
            &graphene::Rect::new(0.0, 0.0, size.x, size.y),
        );

        drop(document);

        if let Some(ref border) = self.state.borrow().border_gizmo {
            self.obj().snapshot_child(border, snapshot);
        }

        snapshot.restore();

        if let Some(ref render) = self.state.borrow().render_area {
            self.obj().snapshot_child(render, snapshot);
        }

        if let Some(ref select) = self.state.borrow().drag_sel_gizmo {
            if select.width() > 1 && select.height() > 1 {
                self.obj().snapshot_child(select, snapshot);
            }
        }

        for (_, gizmo) in self.state.borrow().puppet_gizmos.iter() {
            self.obj().snapshot_child(gizmo, snapshot);
        }

        if let Some(ref resize) = self.state.borrow().selection_gizmo {
            self.obj().snapshot_child(resize, snapshot);
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

        let stage_width = document.stage().size().x;
        let stage_height = document.stage().size().y;

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

        if let Some(ref border) = state.border_gizmo {
            // If we don't measure our children, GTK complains
            border.measure(gtk4::Orientation::Horizontal, stage_width as i32);
            border.allocate(stage_width as i32, stage_height as i32, -1, None);
        }

        drop(document);

        if let Some(ref render) = state.render_area {
            render.measure(gtk4::Orientation::Horizontal, stage_width as i32);
            render.allocate(width as i32, height as i32, -1, None);
        }
    }

    fn set_hadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.hadjustment.borrow_mut() = adjust.clone();

        if let Some(drag_gesture) = &self.state.borrow().drag_gesture {
            drag_gesture.set_hadjustment(adjust);
        }

        self.configure_adjustments();
    }

    fn set_vadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.vadjustment.borrow_mut() = adjust.clone();

        if let Some(drag_gesture) = &self.state.borrow().drag_gesture {
            drag_gesture.set_vadjustment(adjust);
        }

        self.configure_adjustments();
    }

    fn set_zadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.queue_draw();
            });
        }

        *self.zadjustment.borrow_mut() = adjust.clone();

        if let Some(drag_gesture) = &self.state.borrow().drag_gesture {
            drag_gesture.set_zadjustment(adjust.clone());
        }

        if let Some(zoom_gesture) = &self.state.borrow().zoom_gesture {
            zoom_gesture.set_zadjustment(adjust);
        }

        self.configure_adjustments();
    }

    fn update_puppets(&self, dt: f32) {
        //TODO: This should be moved into a separate DocumentManager so that
        //having two windows displaying the same Document doesn't double time
        {
            let state = self.state.borrow_mut();
            let document_arc = state.document.clone();
            let mut document = document_arc.lock().unwrap();

            for (_, puppet) in document.stage_mut().iter_mut() {
                puppet.update(dt);
            }
        }

        self.obj().puppet_updated();
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
        {
            let mut state = self.imp().state.borrow_mut();

            state.document = document.clone();
            state
                .render_area
                .as_ref()
                .unwrap()
                .with_document(document.clone());

            for (_, gizmo) in &state.puppet_gizmos {
                gizmo.unparent();
            }

            state.puppet_gizmos = HashMap::new();

            if let Some(selection_gizmo) = state.selection_gizmo.as_ref() {
                selection_gizmo.set_document(document);
            } else {
                let gizmo = PuppetSelectionGizmo::new();
                gizmo.set_document(document);
                gizmo.set_parent(self);

                state.selection_gizmo = Some(gizmo);
            }
        }

        self.imp().configure_adjustments();
        self.imp().update_puppets(0.0);
    }

    fn bind(&self) {
        self.set_hexpand(true);
        self.set_vexpand(true);
    }

    /// Called by child gizmos whenever they change something with puppets.
    pub fn puppet_updated(&self) {
        let mut state = self.imp().state.borrow_mut();
        let document_arc = state.document.clone();
        let document = document_arc.lock().unwrap();

        //First, collect the garbage.
        document.collect_garbage(&mut state.puppet_gizmos);
        document.collect_garbage_set(&mut state.selected);

        for (index, _puppet) in document.stage().iter() {
            let is_selected = state.selected.contains(&index);

            let gizmo = state.puppet_gizmos.entry(index).or_insert_with(|| {
                let gizmo = PuppetBoundsGizmo::new(document_arc.clone(), index);
                gizmo.set_parent(self);

                gizmo
            });

            if is_selected {
                gizmo.set_state_flags(gtk4::StateFlags::SELECTED, false);
            } else {
                gizmo.unset_state_flags(gtk4::StateFlags::SELECTED);
            }
        }

        drop(document);

        for (_, gizmo) in state.puppet_gizmos.iter() {
            gizmo.puppet_updated(self);
        }

        state.render_area.as_ref().unwrap().queue_render();
        self.queue_draw();

        drop(state);

        self.refresh_selection();
    }

    pub fn set_selected_puppet(&self, puppet: Option<Index>) {
        let mut state = self.imp().state.borrow_mut();

        state.selected = HashSet::new();
        if let Some(puppet) = puppet {
            state.selected.insert(puppet);
        }

        if let Some(resize) = state.selection_gizmo.as_ref() {
            resize.selection_changed(&self, state.selected.iter(), &state.puppet_gizmos);
        }
    }

    pub fn set_selected_to_area(&self, rect: graphene::Rect) {
        let mut state = self.imp().state.borrow_mut();

        let mut selected = HashSet::new();
        for (index, gizmo) in state.puppet_gizmos.iter() {
            let bounds = gizmo.compute_bounds(self);
            if let Some(bounds) = bounds {
                if rect.intersection(&bounds).is_some() {
                    selected.insert(*index);
                }
            }
        }

        state.selected = selected;

        if let Some(resize) = state.selection_gizmo.as_ref() {
            resize.selection_changed(&self, state.selected.iter(), &state.puppet_gizmos);
        }
    }

    /// Called by child gizmos whenever they need to re-measure selections.
    pub fn refresh_selection(&self) {
        let state = self.imp().state.borrow_mut();

        if let Some(resize) = state.selection_gizmo.as_ref() {
            resize.selection_changed(&self, state.selected.iter(), &state.puppet_gizmos);
        }
    }

    /// Given a point on the stage (or off of it), calculate where it should
    /// be relative to this widget's viewport.
    pub fn project_stage_to_viewport(&self, point: Vec2) -> Vec2 {
        let viewport_x = self.imp().hadjustment.borrow().as_ref().unwrap().value() as f32;
        let viewport_y = self.imp().vadjustment.borrow().as_ref().unwrap().value() as f32;
        let scale = 10.0_f64.powf(self.imp().zadjustment.borrow().as_ref().unwrap().value()) as f32;

        Vec2::new(
            (point.x - viewport_x) * scale,
            (point.y - viewport_y) * scale,
        )
    }
}
