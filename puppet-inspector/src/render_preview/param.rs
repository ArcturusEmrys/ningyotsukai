use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use glam::Vec2;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::render_preview::param_list::RenderParamList;
use ningyo_extensions::{StrExt, WidgetExt2};

struct State {
    document: Arc<Mutex<Document>>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/render_preview/param.ui")]
pub struct RenderParamImp {
    state: RefCell<Option<State>>,

    #[template_child]
    param_name: gtk4::TemplateChild<gtk4::Label>,

    #[template_child]
    x_param: gtk4::TemplateChild<gtk4::Adjustment>,
    #[template_child]
    y_param: gtk4::TemplateChild<gtk4::Adjustment>,

    #[template_child]
    x_param_box: gtk4::TemplateChild<gtk4::Box>,
    #[template_child]
    y_param_box: gtk4::TemplateChild<gtk4::Box>,

    #[template_child]
    x_param_scale: gtk4::TemplateChild<gtk4::Scale>,
    #[template_child]
    y_param_scale: gtk4::TemplateChild<gtk4::Scale>,

    #[template_child]
    x_param_label: gtk4::TemplateChild<gtk4::EditableLabel>,
    #[template_child]
    y_param_label: gtk4::TemplateChild<gtk4::EditableLabel>,
}

#[glib::object_subclass]
impl ObjectSubclass for RenderParamImp {
    const NAME: &'static str = "PIRenderParam";
    type Type = RenderParam;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for RenderParamImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for RenderParamImp {}

impl BoxImpl for RenderParamImp {}

glib::wrapper! {
    pub struct RenderParam(ObjectSubclass<RenderParamImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl RenderParam {
    pub fn new(document: Arc<Mutex<Document>>, param: &str) -> Self {
        let selfish: Self = glib::Object::builder().build();

        glib::idle_add_local_once({
            let selfish = selfish.clone();
            let param = param.to_string();
            move || {
                selfish.bind(document, &param);
            }
        });

        selfish
    }

    pub fn bind(&self, document: Arc<Mutex<Document>>, param_name: &str) {
        self.imp().x_param.connect_value_changed({
            let x_self = self.clone();
            let x_param = param_name.to_string();
            move |adj| {
                if !x_self.imp().x_param_label.is_editing() {
                    let rounded = (adj.value() * 100.0).round() / 100.0;
                    x_self
                        .imp()
                        .x_param_label
                        .set_text(&format!("{:.2}", rounded));
                }

                x_self.closest::<RenderParamList>().unwrap().set_override(
                    &x_param,
                    Vec2::new(
                        x_self.imp().x_param.value() as f32,
                        x_self.imp().y_param.value() as f32,
                    ),
                );
            }
        });
        self.imp().y_param.connect_value_changed({
            let y_self = self.clone();
            let y_param = param_name.to_string();
            move |adj| {
                if !y_self.imp().y_param_label.is_editing() {
                    let rounded = (adj.value() * 100.0).round() / 100.0;
                    y_self
                        .imp()
                        .y_param_label
                        .set_text(&format!("{:.2}", rounded));
                }

                y_self.closest::<RenderParamList>().unwrap().set_override(
                    &y_param,
                    Vec2::new(
                        y_self.imp().x_param.value() as f32,
                        y_self.imp().y_param.value() as f32,
                    ),
                );
            }
        });

        // TODO: When we exit editing, we should probably also copy the
        // clamped value back into the label.
        self.imp().x_param_label.connect_text_notify({
            let x_param_label_self = self.clone();
            move |label| {
                if label.is_editing() {
                    if let Ok(val) = label.text().parse::<f64>() {
                        let val = val.clamp(
                            x_param_label_self.imp().x_param.lower(),
                            x_param_label_self.imp().x_param.upper(),
                        );
                        x_param_label_self.imp().x_param.set_value(val);
                    }
                }
            }
        });
        self.imp().y_param_label.connect_text_notify({
            let y_param_label_self = self.clone();
            move |label| {
                if label.is_editing() {
                    if let Ok(val) = label.text().parse::<f64>() {
                        let val = val.clamp(
                            y_param_label_self.imp().y_param.lower(),
                            y_param_label_self.imp().y_param.upper(),
                        );
                        y_param_label_self.imp().y_param.set_value(val);
                    }
                }
            }
        });

        {
            let document_guard = document.lock().unwrap();
            let param = document_guard
                .model
                .puppet
                .params()
                .get(param_name)
                .unwrap();

            self.imp().param_name.set_label(&param.name.escape_nulls());

            self.imp().x_param.set_lower(param.min.x as f64);
            self.imp().y_param.set_lower(param.min.y as f64);

            self.imp().x_param.set_upper(param.max.x as f64);
            self.imp().y_param.set_upper(param.max.y as f64);

            //TODO: This doesn't take into account physics parameters.
            //Ideally we'd have an override button for those?
            self.imp().x_param.set_value(param.defaults.x as f64);
            self.imp().y_param.set_value(param.defaults.y as f64);

            let x_default_rounded = (param.defaults.x * 100.0).round() / 100.0;
            self.imp()
                .x_param_label
                .set_text(&format!("{:.2}", x_default_rounded));

            let y_default_rounded = (param.defaults.y * 100.0).round() / 100.0;
            self.imp()
                .y_param_label
                .set_text(&format!("{:.2}", y_default_rounded));

            let x_delta = param.max.x - param.min.x;
            let y_delta = param.max.y - param.min.y;

            for x_point in param.axis_points.x.iter() {
                self.imp().x_param_scale.add_mark(
                    (*x_point * x_delta + param.min.x) as f64,
                    gtk4::PositionType::Bottom,
                    None,
                );
            }

            for y_point in param.axis_points.y.iter() {
                self.imp().y_param_scale.add_mark(
                    (*y_point * y_delta + param.min.y) as f64,
                    gtk4::PositionType::Bottom,
                    None,
                );
            }

            if !param.is_vec2 {
                self.imp().y_param_box.set_visible(false);
            }
        }

        *self.imp().state.borrow_mut() = Some(State { document });
    }
}
