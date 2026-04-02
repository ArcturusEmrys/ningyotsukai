use gio;
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::tracker::form::{TrackerForm, TrackerFormExt};
use crate::tracker::manager::TrackerManager;
use crate::tracker::reference::{TrackerParamRefItem, TrackerRef, TrackerRefItem};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct State {
    tracker_manager: Rc<TrackerManager>,

    document: Arc<Mutex<Document>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/tracker/panel.ui")]
pub struct TrackerPanelImp {
    state: RefCell<Option<State>>,

    #[template_child]
    tracker_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    tracker_select: gtk4::TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    tracker_model: gtk4::TemplateChild<gio::ListStore>,

    #[template_child]
    new_button: gtk4::TemplateChild<gtk4::Button>,
    #[template_child]
    delete_button: gtk4::TemplateChild<gtk4::Button>,
    #[template_child]
    edit_button: gtk4::TemplateChild<gtk4::Button>,

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
impl ObjectSubclass for TrackerPanelImp {
    const NAME: &'static str = "NGTTrackerPanel";
    type Type = TrackerPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for TrackerPanelImp {
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

        let new_button_self = self.obj().clone();
        self.new_button.connect_clicked(move |_| {
            let form_window = TrackerForm::new();

            form_window.set_modal(true);
            form_window.set_title(Some("New tracker"));

            let form_connect_self = new_button_self.clone();
            form_window.connect_save(move |form, tracker| {
                let state = form_connect_self.imp().state.borrow();
                let state = state.as_ref().unwrap();
                let mut document = state.document.lock().unwrap();

                let tracker_id = document.trackers_mut().register(tracker);

                drop(document);

                state
                    .tracker_manager
                    .register_tracker(TrackerRef::new(&state.document, tracker_id));

                form.destroy();

                form_connect_self.imp().populate_list();
            });

            form_window.connect_cancel(|w| {
                w.destroy();
            });

            form_window.present();
        });

        let edit_button_self = self.obj().clone();
        self.edit_button.connect_clicked(move |_| {
            if let Some(tracker_ref) = edit_button_self.imp().tracker_select.selected_item() {
                let tracker_ref = tracker_ref.downcast_ref::<TrackerRefItem>().unwrap();
                let tracker_ref = tracker_ref.contents();

                let form_window = TrackerForm::new();

                form_window.set_modal(true);
                form_window.set_title(Some("Edit tracker"));
                tracker_ref.with_tracker(|tracker| {
                    form_window.populate_with_tracker(tracker);
                });

                let form_connect_self = edit_button_self.clone();
                form_window.connect_save(move |form, tracker| {
                    // For various reasons, we treat trackers as immutable
                    // once registered, so this is delete-and-create
                    let state = form_connect_self.imp().state.borrow();
                    let state = state.as_ref().unwrap();

                    state.tracker_manager.unregister_tracker(TrackerRef::new(
                        &state.document,
                        tracker_ref.tracker_index(),
                    ));

                    let mut document = state.document.lock().unwrap();

                    document
                        .trackers_mut()
                        .unregister(tracker_ref.tracker_index());
                    let new_tracker_id = document.trackers_mut().register(tracker);

                    drop(document);

                    state
                        .tracker_manager
                        .register_tracker(TrackerRef::new(&state.document, new_tracker_id));

                    form.destroy();

                    form_connect_self.imp().populate_list();
                });

                form_window.connect_cancel(|w| {
                    w.destroy();
                });

                form_window.present();
            }
        });

        let delete_button_self = self.obj().clone();
        self.delete_button.connect_clicked(move |_| {
            if let Some(tracker_ref) = delete_button_self.imp().tracker_select.selected_item() {
                let tracker_ref = tracker_ref.downcast_ref::<TrackerRefItem>().unwrap();
                let tracker_ref = tracker_ref.contents();
                let state = delete_button_self.imp().state.borrow();
                let state = state.as_ref().unwrap();

                state.tracker_manager.unregister_tracker(TrackerRef::new(
                    &state.document,
                    tracker_ref.tracker_index(),
                ));

                let mut document = state.document.lock().unwrap();

                document
                    .trackers_mut()
                    .unregister(tracker_ref.tracker_index());

                drop(document);

                delete_button_self.imp().populate_list();
            }
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

            if let Some(value) = tracker_param_ref_item.contents().value() {
                label.set_label(&format!("{}", value));
            }
        });
    }
}

impl WidgetImpl for TrackerPanelImp {
    fn unrealize(&self) {
        //We need to drop our tracker manager, otherwise we keep the application open
        self.state.borrow_mut().take();
        self.parent_unrealize();
    }
}

impl BoxImpl for TrackerPanelImp {}

impl TrackerPanelImp {
    fn populate_list(&self) {
        self.tracker_model.remove_all();

        let state = self.state.borrow();
        let state = state.as_ref().unwrap();

        let trackers = {
            let document = state.document.lock().unwrap();
            let mut trackers = vec![];

            for (index, _tracker) in document.trackers().iter() {
                let tracker_ref = TrackerRefItem::from(TrackerRef::new(&state.document, index));
                trackers.push(tracker_ref);
            }

            trackers
        };

        self.tracker_model.extend_from_slice(&trackers);
    }
}

glib::wrapper! {
    pub struct TrackerPanel(ObjectSubclass<TrackerPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl TrackerPanel {
    pub fn bind(&self, tracker_manager: Rc<TrackerManager>, document: Arc<Mutex<Document>>) {
        // Don't ask why, but calling connect_params_changed on stack panics.
        let idle_self = self.clone();
        let idle_tm = tracker_manager.clone();
        glib::idle_add_local_once(move || {
            let tracker_manager_self = idle_self.clone().downgrade();
            idle_tm.connect_params_changed(move || {
                if let Some(tracker_manager_self) = tracker_manager_self.upgrade() {
                    tracker_manager_self.tracker_params_updated();
                    return glib::ControlFlow::Continue;
                }

                glib::ControlFlow::Break
            });
        });

        *self.imp().state.borrow_mut() = Some(State {
            tracker_manager,
            document,
        });

        self.imp().populate_list();
    }

    pub fn tracker_params_updated(&self) {
        let mut refs: Vec<TrackerParamRefItem> = vec![];

        if let Some(item) = self.imp().tracker_select.selected_item() {
            let tracker_ref_item = item.downcast_ref::<TrackerRefItem>().unwrap();
            let tracker_ref = tracker_ref_item.contents();

            let document = tracker_ref.document().unwrap();
            let document = document.lock().unwrap();
            if let Some(data) = document.trackers().data(tracker_ref.tracker_index()) {
                for (name, datatype) in data.iter_params() {
                    refs.push(tracker_ref.with_param_ref(name, datatype).into());
                }
            }
        }

        self.imp().param_model.remove_all();
        self.imp().param_model.extend_from_slice(refs.as_slice());
    }
}
