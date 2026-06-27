use std::cell::RefCell;
use std::str::FromStr;

use glib::subclass::InitializingObject;
use glib::{Properties, WeakRef};
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use generational_arena::Index;
use ningyo_extensions::{StrExt, WidgetExt2};

use crate::document::Document;
use crate::stage::{StageWidget, StageWidgetExt};

struct State {
    document: Document,

    puppet: Index,

    stage: WeakRef<StageWidget>,
}

#[derive(CompositeTemplate, Default, Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/inspector/puppet.ui")]
#[properties(wrapper_type=PuppetInspector)]
pub struct PuppetInspectorImp {
    state: RefCell<Option<State>>,

    #[template_child]
    name: gtk4::TemplateChild<gtk4::Label>,

    #[template_child]
    position_x_entry: gtk4::TemplateChild<gtk4::Entry>,

    #[template_child]
    position_y_entry: gtk4::TemplateChild<gtk4::Entry>,

    #[template_child]
    width_entry: gtk4::TemplateChild<gtk4::Entry>,

    #[template_child]
    height_entry: gtk4::TemplateChild<gtk4::Entry>,

    #[template_child]
    scale_entry: gtk4::TemplateChild<gtk4::Entry>,
}

#[glib::object_subclass]
impl ObjectSubclass for PuppetInspectorImp {
    const NAME: &'static str = "NGTPuppetInspector";
    type Type = PuppetInspector;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for PuppetInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for PuppetInspectorImp {}

impl BoxImpl for PuppetInspectorImp {}

impl PuppetInspectorImp {
    fn puppet_updated(&self) {
        let state_outer = self.state.borrow();
        let state = state_outer.as_ref().unwrap();

        let (x, y, scale, w, h) = if let Some(puppet) = state.document.stage().puppet(state.puppet)
        {
            if let Some(name) = &puppet.model().puppet.meta.name {
                self.name.set_label(&name.escape_nulls());
            }

            (
                puppet.position().x,
                puppet.position().y,
                puppet.scale(),
                puppet.bounds().as_ref().map(|b| b.width()).unwrap_or(0.0) * puppet.scale(),
                puppet.bounds().as_ref().map(|b| b.height()).unwrap_or(0.0) * puppet.scale(),
            )
        } else {
            return;
        };

        drop(state_outer);

        let x = format!("{}", x);
        if self.position_x_entry.buffer().text() != x
            && !self.position_x_entry.has_transitive_focus()
        {
            self.position_x_entry.buffer().set_text(x);
        }

        let y = format!("{}", y);
        if self.position_y_entry.buffer().text() != y
            && !self.position_y_entry.has_transitive_focus()
        {
            self.position_y_entry.buffer().set_text(y);
        }

        let scale = format!("{}", scale);
        if self.scale_entry.buffer().text() != scale && !self.scale_entry.has_transitive_focus() {
            self.scale_entry.buffer().set_text(scale);
        }

        let w = format!("{}", w);
        if self.width_entry.buffer().text() != w && !self.width_entry.has_transitive_focus() {
            self.width_entry.buffer().set_text(w);
        }

        let h = format!("{}", h);
        if self.height_entry.buffer().text() != h && !self.height_entry.has_transitive_focus() {
            self.height_entry.buffer().set_text(h);
        }
    }
}

glib::wrapper! {
    pub struct PuppetInspector(ObjectSubclass<PuppetInspectorImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl PuppetInspector {
    pub fn new(document: Document, puppet: Index, stage: StageWidget) -> Self {
        let selfish: PuppetInspector = glib::Object::builder().build();

        selfish.bind(document, puppet, stage);

        selfish
    }

    pub fn bind(&self, document: Document, puppet: Index, stage: StageWidget) {
        *self.imp().state.borrow_mut() = Some(State {
            document: document.clone(),
            puppet,
            stage: stage.downgrade(),
        });

        self.imp().puppet_updated();

        stage.connect_updated({
            let stage_self = self.clone();
            move |_| {
                stage_self.imp().puppet_updated();
            }
        });

        self.imp().position_x_entry.connect_changed({
            let pos_x_self = self.clone();
            move |field| {
                if !field.has_transitive_focus() {
                    return;
                }

                if let Ok(x_pos) = f32::from_str(&field.buffer().text()) {
                    let mut state = pos_x_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if let Some(mut puppet) = state.document.stage_mut().puppet(state.puppet) {
                        puppet.set_position(glam::Vec2 {
                            x: x_pos,
                            y: puppet.position().y,
                        });
                    }
                }
            }
        });

        self.imp().position_y_entry.connect_changed({
            let pos_y_self = self.clone();
            move |field| {
                if !field.has_transitive_focus() {
                    return;
                }

                if let Ok(y_pos) = f32::from_str(&field.buffer().text()) {
                    let mut state = pos_y_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if let Some(mut puppet) = state.document.stage_mut().puppet(state.puppet) {
                        puppet.set_position(glam::Vec2 {
                            x: puppet.position().x,
                            y: y_pos,
                        });
                    }
                }
            }
        });

        self.imp().scale_entry.connect_changed({
            let scale_self = self.clone();
            move |field| {
                if !field.has_transitive_focus() {
                    return;
                }

                if let Ok(scale) = f32::from_str(&field.buffer().text()) {
                    let mut state = scale_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if let Some(mut puppet) = state.document.stage_mut().puppet(state.puppet) {
                        puppet.set_scale(scale);
                    }
                }
            }
        });

        self.imp().width_entry.connect_changed({
            let width_self = self.clone();
            move |field| {
                if !field.has_transitive_focus() {
                    return;
                }

                if let Ok(new_width) = f32::from_str(&field.buffer().text()) {
                    let mut state = width_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if let Some(mut puppet) = state.document.stage_mut().puppet(state.puppet) {
                        let scale = if let Some(bounds) = puppet.bounds() {
                            let bounds_width = bounds.width();
                            new_width / bounds_width
                        } else {
                            return;
                        };

                        puppet.set_scale(scale);
                    }
                }
            }
        });

        self.imp().height_entry.connect_changed({
            let height_self = self.clone();
            move |field| {
                if !field.has_transitive_focus() {
                    return;
                }

                if let Ok(new_height) = f32::from_str(&field.buffer().text()) {
                    let mut state = height_self.imp().state.borrow_mut();
                    let state = state.as_mut().unwrap();
                    if let Some(mut puppet) = state.document.stage_mut().puppet(state.puppet) {
                        let scale = if let Some(bounds) = puppet.bounds() {
                            let bounds_height = bounds.height();
                            new_height / bounds_height
                        } else {
                            return;
                        };

                        puppet.set_scale(scale);
                    }
                }
            }
        });
    }
}
