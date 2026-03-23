use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::document::model::Document;
use crate::panels::PanelDock;
use crate::stage::{Puppet, StageWidget};

use ningyo_extensions::{FileIn, WidgetExt2};

#[derive(Default)]
pub struct DocumentControllerState {
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

    state: RefCell<DocumentControllerState>,
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

        self.stage
            .set_document(self.state.borrow().document.clone());

        // Wire up document specific actions
        let doc = gio::SimpleActionGroup::new();

        let doc_controller_import_puppet = self.obj().clone();
        doc.add_action_entries([gio::ActionEntry::builder("import-puppet")
            .activate(move |_, _, _| {
                let callback_self = doc_controller_import_puppet.clone();
                doc_controller_import_puppet.imp().filepicker_puppet.open(
                    doc_controller_import_puppet.window().as_ref(),
                    Some(&gio::Cancellable::new()),
                    move |file_or_error| {
                        let maybe_error: Result<(), Box<dyn Error>> = (|| {
                            callback_self.open_document(file_or_error?)?;
                            Ok(())
                        })();

                        if let Err(e) = maybe_error {
                            eprintln!("{:?}", e);
                        }
                    },
                )
            })
            .build()]);

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

impl WidgetImpl for DocumentControllerImp {}

impl BoxImpl for DocumentControllerImp {}

glib::wrapper! {
    pub struct DocumentController(ObjectSubclass<DocumentControllerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl DocumentController {
    pub fn new(app: &gtk4::Application) -> Self {
        let selfish: DocumentController =
            glib::Object::builder().property("application", app).build();

        selfish
    }

    pub fn open_document(&self, file: gio::File) -> Result<(), Box<dyn Error>> {
        let stream = file.read(Some(&gio::Cancellable::new()))?;
        let stream_adapter = FileIn::from(stream);

        let puppet = Puppet::open(stream_adapter)?;
        let state = self.imp().state.borrow_mut();
        let mut document = state.document.lock().unwrap();

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
