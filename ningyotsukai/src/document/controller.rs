use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use crate::bindings::BindingPanel;
use crate::document::DocumentManager;
use crate::document::model::Document;
use crate::panels::PanelDock;
use crate::panels::PanelFrame;
use crate::stage::{Puppet, StageWidget};
use crate::tracker::{TrackerManager, TrackerPanel, TrackerParamPanel};

use ningyo_extensions::{FileIn, WidgetExt2};

pub struct DocumentControllerState {
    tracker_manager: Rc<TrackerManager>,
    document_manager: DocumentManager,
    document: Document,
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
        doc.add_action_entries([
            gio::ActionEntry::builder("import-puppet")
                .activate({
                    let callback_self = self.obj().clone();
                    move |_, _, _| {
                        let callback2_self = callback_self.clone();
                        callback_self.imp().filepicker_puppet.open(
                            callback_self.window().as_ref(),
                            Some(&gio::Cancellable::new()),
                            move |file_or_error| {
                                let maybe_error: Result<(), Box<dyn Error>> = (|| {
                                    callback2_self.import_puppet(file_or_error?)?;
                                    Ok(())
                                })(
                                );

                                if let Err(e) = maybe_error {
                                    eprintln!("{:?}", e);
                                }
                            },
                        )
                    }
                })
                .build(),
            gio::ActionEntry::builder("panels-tracker")
                .state(false.into())
                .activate({
                    let callback_self = self.obj().clone();
                    move |_, action, _| {
                        let panel_open: bool = action.state().unwrap().get().unwrap();

                        if !panel_open {
                            let state = callback_self.imp().state.borrow();
                            let tracker_manager = state.as_ref().unwrap().tracker_manager.clone();
                            let document = state.as_ref().unwrap().document.clone();
                            let builder = gtk4::Builder::from_resource(
                                "/live/arcturus/ningyotsukai/tracker/panel_frame.ui",
                            );
                            let panel: PanelFrame = builder.object("panel").unwrap();
                            let contents: TrackerPanel = builder.object("contents").unwrap();

                            contents.bind(tracker_manager, document);

                            callback_self.imp().new_panel_dock.append(&panel);
                        } else {
                            for panel_frame in callback_self.find_all::<PanelFrame>() {
                                if panel_frame.remove_page_by_action("doc.panels-tracker") {
                                    break;
                                }
                            }
                        }

                        action.set_state(&glib::Variant::from(!panel_open));
                    }
                })
                .build(),
            gio::ActionEntry::builder("panels-tracker-params")
                .state(false.into())
                .activate({
                    let callback_self = self.obj().clone();
                    move |_, action, _| {
                        let panel_open: bool = action.state().unwrap().get().unwrap();

                        if !panel_open {
                            let state = callback_self.imp().state.borrow();
                            let tracker_manager = state.as_ref().unwrap().tracker_manager.clone();
                            let document = state.as_ref().unwrap().document.clone();
                            let builder = gtk4::Builder::from_resource(
                                "/live/arcturus/ningyotsukai/tracker/params/panel_frame.ui",
                            );
                            let panel: PanelFrame = builder.object("panel").unwrap();
                            let contents: TrackerParamPanel = builder.object("contents").unwrap();

                            contents.bind(tracker_manager, document);

                            callback_self.imp().new_panel_dock.append(&panel);
                        } else {
                            for panel_frame in callback_self.find_all::<PanelFrame>() {
                                if panel_frame.remove_page_by_action("doc.panels-tracker-params") {
                                    break;
                                }
                            }
                        }

                        action.set_state(&glib::Variant::from(!panel_open));
                    }
                })
                .build(),
            gio::ActionEntry::builder("panels-bindings")
                .state(false.into())
                .activate({
                    let callback_self = self.obj().clone();
                    move |_, action, _| {
                        let panel_open: bool = action.state().unwrap().get().unwrap();

                        if !panel_open {
                            let state = callback_self.imp().state.borrow();
                            let document = state.as_ref().unwrap().document.clone();
                            let builder = gtk4::Builder::from_resource(
                                "/live/arcturus/ningyotsukai/bindings/panel_frame.ui",
                            );
                            let panel: PanelFrame = builder.object("panel").unwrap();
                            let contents: BindingPanel = builder.object("contents").unwrap();

                            contents.bind(document, callback_self.imp().stage.clone());

                            callback_self.imp().new_panel_dock.append(&panel);
                        } else {
                            for panel_frame in callback_self.find_all::<PanelFrame>() {
                                if panel_frame.remove_page_by_action("doc.panels-bindings") {
                                    break;
                                }
                            }
                        }

                        action.set_state(&glib::Variant::from(!panel_open));
                    }
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
    pub fn bind(
        &self,
        tracker_manager: Rc<TrackerManager>,
        mut document_manager: DocumentManager,
        document: Document,
    ) {
        if let Some(DocumentControllerState {
            document,
            document_manager,
            ..
        }) = &mut *self.imp().state.borrow_mut()
        {
            document_manager.unregister_document(document.clone());
        }

        *self.imp().state.borrow_mut() = Some(DocumentControllerState {
            tracker_manager,
            document_manager: document_manager.clone(),
            document: document.clone(),
        });

        document_manager.register_document(document.clone());
        self.imp().stage.set_document(document, document_manager);
    }

    pub fn import_puppet(&self, file: gio::File) -> Result<(), Box<dyn Error>> {
        let stream = file.read(Some(&gio::Cancellable::new()))?;
        let stream_adapter = FileIn::from(stream);

        let mut puppet = Puppet::open(stream_adapter)?;

        puppet.ensure_render_initialized();

        let mut state = self.imp().state.borrow_mut();
        let document = &mut state.as_mut().unwrap().document;

        // This heuristic is here to ensure very large puppets get scaled down
        // to something reasonable.
        let bounds = puppet.model().puppet.bounds();
        if let Some(bounds) = bounds {
            let longer_bounds_length = bounds.width().max(bounds.height());
            let shorter_stage_length = document.stage().size().x.min(document.stage().size().y);

            let recommended_zoom = shorter_stage_length / longer_bounds_length;

            puppet.set_scale(recommended_zoom);
        }

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
