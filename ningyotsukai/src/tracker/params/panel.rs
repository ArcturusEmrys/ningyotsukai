use gio;
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::tracker::manager::TrackerManager;
use crate::tracker::reference::{TrackerParamRefItem, TrackerRef, TrackerRefItem};

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

struct State {
    document: Document,

    pending_update: Option<glib::SourceId>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/tracker/params/panel.ui")]
pub struct TrackerParamPanelImp {
    state: RefCell<Option<State>>,

    #[template_child]
    tracker_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    tracker_select: gtk4::TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    tracker_model: gtk4::TemplateChild<gio::ListStore>,

    #[template_child]
    param_name_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    param_type_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    param_value_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    param_select: gtk4::TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    param_model: gtk4::TemplateChild<gio::ListStore>,
}

#[glib::object_subclass]
impl ObjectSubclass for TrackerParamPanelImp {
    const NAME: &'static str = "NGTTrackerParamPanel";
    type Type = TrackerParamPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for TrackerParamPanelImp {
    fn constructed(&self) {
        self.parent_constructed();

        self.tracker_factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk4::ListItem>() {
                item.set_child(Some(&gtk4::Label::builder().label("test").build()));
            }
        });

        self.tracker_factory.connect_bind(|_, item| {
            let list_item = item.downcast_ref::<gtk4::ListItem>().unwrap();
            let item = list_item.item().unwrap();
            let tracker_ref = item.downcast_ref::<TrackerRefItem>().unwrap();
            let child = list_item.child().unwrap();
            let label = child.downcast_ref::<gtk4::Label>().unwrap();

            tracker_ref
                .contents()
                .with_tracker(|tracker| label.set_label(tracker.name()));
        });

        let tracker_select_selected_self = self.obj().clone();
        self.tracker_select.connect_selected_notify(move |_select| {
            tracker_select_selected_self.tracker_params_updated();
        });

        self.param_name_factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = gtk4::Label::builder().build();

            list_item.set_child(Some(&label));
        });

        self.param_name_factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = list_item
                .child()
                .unwrap()
                .downcast::<gtk4::Label>()
                .unwrap();

            let item = list_item.item().unwrap();
            let tracker_param_ref_item = item.downcast_ref::<TrackerParamRefItem>().unwrap();

            label.set_label(tracker_param_ref_item.contents().param_name());
        });

        self.param_type_factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = gtk4::Label::builder().build();

            list_item.set_child(Some(&label));
        });

        self.param_type_factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = list_item
                .child()
                .unwrap()
                .downcast::<gtk4::Label>()
                .unwrap();

            let item = list_item.item().unwrap();
            let tracker_param_ref_item = item.downcast_ref::<TrackerParamRefItem>().unwrap();

            label.set_label(tracker_param_ref_item.contents().param_datatype());
        });

        self.param_value_factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = gtk4::Label::builder().build();

            list_item.set_child(Some(&label));
        });

        self.param_value_factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            let label = list_item
                .child()
                .unwrap()
                .downcast::<gtk4::Label>()
                .unwrap();

            let item = list_item.item().unwrap();
            let tracker_param_ref_item = item.downcast_ref::<TrackerParamRefItem>().unwrap();

            label.set_label(&format!("{}", tracker_param_ref_item.contents().value()));
        });
    }
}

impl WidgetImpl for TrackerParamPanelImp {}

impl BoxImpl for TrackerParamPanelImp {}

impl TrackerParamPanelImp {
    fn populate_list(&self) {
        self.tracker_model.remove_all();

        let state = self.state.borrow();
        let state = state.as_ref().unwrap();

        let trackers = {
            let mut trackers = vec![];

            for (index, _tracker) in state.document.trackers().iter() {
                let tracker_ref = TrackerRefItem::from(TrackerRef::new(&state.document, index));
                trackers.push(tracker_ref);
            }

            trackers
        };

        self.tracker_model.extend_from_slice(&trackers);
    }
}

glib::wrapper! {
    pub struct TrackerParamPanel(ObjectSubclass<TrackerParamPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl TrackerParamPanel {
    pub fn bind(&self, tracker_manager: Rc<TrackerManager>, document: Document) {
        // Don't ask why, but calling connect_params_changed on stack panics.
        let idle_self = self.clone();
        let idle_tm = tracker_manager.clone();
        glib::idle_add_local_once(move || {
            let tracker_manager_self = idle_self.clone().downgrade();
            idle_tm.connect_params_changed(move || {
                if let Some(tracker_manager_self) = tracker_manager_self.upgrade() {
                    let mut state = tracker_manager_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if state.pending_update.is_some() {
                        return glib::ControlFlow::Continue;
                    }

                    let pending_self = tracker_manager_self.clone();
                    state.pending_update = Some(glib::timeout_add_local_once(
                        Duration::new(0, 100_000_000),
                        move || {
                            let mut state2 = pending_self.imp().state.borrow_mut();
                            let state = state2.as_mut().unwrap();

                            state.pending_update = None;
                            drop(state2);

                            pending_self.imp().populate_list();
                            pending_self.tracker_params_updated();
                        },
                    ));

                    return glib::ControlFlow::Continue;
                }

                glib::ControlFlow::Break
            });
        });

        *self.imp().state.borrow_mut() = Some(State {
            document,
            pending_update: None,
        });

        self.imp().populate_list();
    }

    pub fn tracker_params_updated(&self) {
        let mut refs: Vec<TrackerParamRefItem> = vec![];

        if let Some(item) = self.imp().tracker_select.selected_item() {
            let tracker_ref_item = item.downcast_ref::<TrackerRefItem>().unwrap();
            let tracker_ref = tracker_ref_item.contents();

            let document = tracker_ref.document().unwrap();
            if let Some(data) = document.trackers().data(tracker_ref.tracker_index()) {
                for (name, datatype) in data.iter_params() {
                    if let Some(value) = data.value(name, datatype) {
                        refs.push(tracker_ref.with_param(name, datatype, value).into());
                    }
                }
            }
        }

        self.imp().param_model.remove_all();
        self.imp().param_model.extend_from_slice(refs.as_slice());
    }
}
