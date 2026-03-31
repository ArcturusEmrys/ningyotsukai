use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::document::model::Document;
use crate::panels::PanelDock;
use crate::panels::PanelFrame;
use crate::stage::{Puppet, StageWidget};
use crate::tracker::{TrackerManager, TrackerPanel};

use ningyo_extensions::{FileIn, WidgetExt2};

pub struct DocumentControllerState {
    tracker_manager: Rc<TrackerManager>,
    document: Arc<Mutex<Document>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/document/controller.ui")]
pub struct DocumentControllerImp {
    #[template_child]
    stage: TemplateChild<StageWidget>,
    #[template_child]
    zoom_label: TemplateChild<gtk4::EditableLabel>,
    #[template_child]
    zoom_adjust: TemplateChild<gtk4::Adjustment>,
    #[template_child]
    filepicker_puppet: TemplateChild<gtk4::FileDialog>,
    #[template_child]
    new_panel_dock: TemplateChild<PanelDock>,

    state: RefCell<Option<DocumentControllerState>>,
}

#[glib::object_subclass]
impl ObjectSubclass for DocumentControllerImp {
    const NAME: &'static str = "NGTDocumentController";
    type Type = DocumentController;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for DocumentControllerImp {
    fn constructed(&self) {
        self.parent_constructed();

        // Wire up document specific actions
        let doc = gio::SimpleActionGroup::new();

        let doc_controller_import_puppet = self.obj().clone();
        let doc_controller_panels_tracker = self.obj().clone();
        doc.add_action_entries([
            gio::ActionEntry::builder("import-puppet")
                .activate(move |_, _, _| {
                    let callback_self = doc_controller_import_puppet.clone();
                    doc_controller_import_puppet.imp().filepicker_puppet.open(
                        doc_controller_import_puppet.window().as_ref(),
                        Some(&gio::Cancellable::new()),
                        move |file_or_error| {
                            let maybe_error: Result<(), Box<dyn Error>> = (|| {
                                callback_self.import_puppet(file_or_error?)?;
                                Ok(())
                            })(
                            );

                            if let Err(e) = maybe_error {
                                eprintln!("{:?}", e);
                            }
                        },
                    )
                })
                .build(),
            gio::ActionEntry::builder("panels-tracker")
                .state(false.into())
                .activate(move |_, action, _| {
                    let panel_open: bool = action.state().unwrap().get().unwrap();

                    if !panel_open {
                        let state = doc_controller_panels_tracker.imp().state.borrow();
                        let tracker_manager = state.as_ref().unwrap().tracker_manager.clone();
                        let document = state.as_ref().unwrap().document.clone();
                        let builder = gtk4::Builder::from_resource(
                            "/live/arcturus/ningyotsukai/tracker/panel_frame.ui",
                        );
                        let panel: PanelFrame = builder.object("panel").unwrap();
                        let contents: TrackerPanel = builder.object("contents").unwrap();

                        contents.bind(tracker_manager, document);

                        doc_controller_panels_tracker
                            .imp()
                            .new_panel_dock
                            .append(&panel);

                        action.set_state(&glib::Variant::from(!panel_open));
                    }
                    // TODO: Allow closing the panel from the menu item.
                })
                .build(),
        ]);

        self.obj().connect_realize(move |selfpoi| {
            selfpoi
                .window()
                .unwrap()
                .insert_action_group("doc", Some(&doc));
        });

        self.zoom_label.set_text(&format!(
            "{:.0}%",
            10.0_f64.powf(self.zoom_adjust.value()) * 100.0
        ));

        let zoom_adjust_self = self.obj().clone();
        self.zoom_adjust.connect_value_changed(move |adj| {
            zoom_adjust_self
                .imp()
                .zoom_label
                .set_text(&format!("{:.0}%", 10.0_f64.powf(adj.value()) * 100.0));
        });

        let zoom_label_self = self.obj().clone();
        self.zoom_label.connect_editing_notify(move |label| {
            if label.is_editing() {
                //Do nothing until the user enters a value.
                return;
            }

            let label_value = label.text();
            let mut label_value = label_value.trim();
            if label_value.ends_with("%") {
                label_value = label_value.trim_end_matches("%");
            }

            let value = if let Ok(value) = label_value.parse::<f64>() {
                (value / 100.0).log(10.0)
            } else {
                f64::NAN
            };

            if value.is_finite() && !value.is_nan() {
                zoom_label_self.imp().zoom_adjust.set_value(value);
            } else {
                //This resets the label, so the user can try again
                zoom_label_self
                    .imp()
                    .zoom_adjust
                    .set_value(zoom_label_self.imp().zoom_adjust.value());
            }
        });
    }
}

impl WidgetImpl for DocumentControllerImp {
    fn unrealize(&self) {
        //We need to drop our tracker manager, otherwise we keep the application open
        self.state.borrow_mut().take();
        self.parent_unrealize();
    }
}

impl BoxImpl for DocumentControllerImp {}

glib::wrapper! {
    pub struct DocumentController(ObjectSubclass<DocumentControllerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl DocumentController {
    pub fn bind(&self, tracker_manager: Rc<TrackerManager>, document: Arc<Mutex<Document>>) {
        *self.imp().state.borrow_mut() = Some(DocumentControllerState {
            tracker_manager,
            document: document.clone(),
        });

        self.imp().stage.set_document(document);
    }

    pub fn import_puppet(&self, file: gio::File) -> Result<(), Box<dyn Error>> {
        let stream = file.read(Some(&gio::Cancellable::new()))?;
        let stream_adapter = FileIn::from(stream);

        let puppet = Puppet::open(stream_adapter)?;
        let state = self.imp().state.borrow_mut();
        let mut document = state.as_ref().unwrap().document.lock().unwrap();

        document.stage_mut().add_puppet(puppet);

        self.imp().stage.queue_draw();

        Ok(())
    }

    pub fn panel_drag_began(&self) {
        for dock in self.find_all::<PanelDock>() {
            dock.panel_drag_began();
        }
    }

    pub fn panel_drag_ended(&self) {
        for dock in self.find_all::<PanelDock>() {
            dock.panel_drag_ended();
        }
    }
}
