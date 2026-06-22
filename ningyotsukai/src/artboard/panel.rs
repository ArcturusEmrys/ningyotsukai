use std::cell::RefCell;
use std::mem::swap;
use std::str::FromStr;

use glib::subclass::InitializingObject;
use glib::{Properties, WeakRef};
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::stage::StageWidget;

struct State {
    document: Document,

    stage: WeakRef<StageWidget>,
}

#[derive(CompositeTemplate, Default, Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/artboard/panel.ui")]
#[properties(wrapper_type=ArtboardPanel)]
pub struct ArtboardPanelImp {
    state: RefCell<Option<State>>,

    #[template_child]
    width_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    height_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    aspect_ratio_flip: gtk4::TemplateChild<gtk4::Button>,

    #[property(name="artboard_width", get=Self::artboard_width, set=Self::set_artboard_width)]
    width: RefCell<f32>,
    #[property(name="artboard_height", get=Self::artboard_height, set=Self::set_artboard_height)]
    height: RefCell<f32>,
}

#[glib::object_subclass]
impl ObjectSubclass for ArtboardPanelImp {
    const NAME: &'static str = "NGTArtboardPanel";
    type Type = ArtboardPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ArtboardPanelImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for ArtboardPanelImp {}

impl BoxImpl for ArtboardPanelImp {}

impl ArtboardPanelImp {
    fn artboard_width(&self) -> f32 {
        *self.width.borrow()
    }

    fn set_artboard_width(&self, value: f32) {
        *self.width.borrow_mut() = value;
    }

    fn artboard_height(&self) -> f32 {
        *self.height.borrow()
    }

    fn set_artboard_height(&self, value: f32) {
        *self.height.borrow_mut() = value;
    }
}

glib::wrapper! {
    pub struct ArtboardPanel(ObjectSubclass<ArtboardPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl ArtboardPanel {
    pub fn bind(&self, document: Document, stage: StageWidget) {
        *self.imp().state.borrow_mut() = Some(State {
            document,
            stage: stage.downgrade(),
        });

        let current_size = self
            .imp()
            .state
            .borrow()
            .as_ref()
            .unwrap()
            .document
            .stage()
            .size();

        self.imp()
            .width_entry
            .buffer()
            .set_text(format!("{}", current_size.x));
        self.imp()
            .height_entry
            .buffer()
            .set_text(format!("{}", current_size.y));

        self.imp().width_entry.connect_changed({
            let width_self = self.clone();
            move |entry| {
                let mut size = width_self
                    .imp()
                    .state
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .document
                    .stage()
                    .size();

                if let Ok(width) = f32::from_str(&entry.buffer().text()) {
                    if size.x == width {
                        return;
                    }

                    size.x = width;
                }

                width_self
                    .imp()
                    .state
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .document
                    .stage_mut()
                    .set_size(size);

                width_self
                    .size_changed();
            }
        });

        self.imp().height_entry.connect_changed({
            let height_self = self.clone();
            move |entry| {
                let mut size = height_self
                    .imp()
                    .state
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .document
                    .stage()
                    .size();

                if let Ok(height) = f32::from_str(&entry.buffer().text()) {
                    if size.y == height {
                        return;
                    }

                    size.y = height;
                }

                height_self
                    .imp()
                    .state
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .document
                    .stage_mut()
                    .set_size(size);

                height_self
                    .size_changed();
            }
        });

        self.imp().aspect_ratio_flip.connect_clicked({
            let arf_self = self.clone();
            move |_| {
                let mut size = arf_self
                    .imp()
                    .state
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .document
                    .stage()
                    .size();

                swap(&mut size.x, &mut size.y);

                arf_self
                    .imp()
                    .state
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .document
                    .stage_mut()
                    .set_size(size);

                arf_self.size_changed();
            }
        });
    }

    fn size_changed(&self) {
        let size = self.imp().state.borrow().as_ref().unwrap().document.stage().size();

        let width_text = format!("{}", size.x);
        if self.imp().width_entry.buffer().text() != width_text {
            self.imp().width_entry.buffer().set_text(width_text);
        }

        let height_text = format!("{}", size.y);
        if self.imp().height_entry.buffer().text() != height_text {
            self.imp().height_entry.buffer().set_text(height_text);
        }

        self.imp().state.borrow().as_ref().unwrap().stage.upgrade().unwrap().stage_resized();
    }
}
