use glib::Properties;
use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::str::FromStr;

#[derive(CompositeTemplate, Default, Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/bindings/form.ui")]
#[properties(wrapper_type=BindingForm)]
pub struct BindingFormImp {
    #[template_child]
    name: gtk4::TemplateChild<gtk4::Label>,
    #[template_child]
    tracker_param_factory: gtk4::TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    tracker_param_select: gtk4::TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    tracker_param_model: gtk4::TemplateChild<gio::ListStore>,
    #[template_child]
    dampening_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    value_in_from_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    value_in_to_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    value_in_display: gtk4::TemplateChild<gtk4::LevelBar>,
    #[template_child]
    value_out_from_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    value_out_to_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    value_out_display: gtk4::TemplateChild<gtk4::LevelBar>,
    #[template_child]
    value_invert_check: gtk4::TemplateChild<gtk4::CheckButton>,
    #[template_child]
    expression_entry: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    expression_error_label: gtk4::TemplateChild<gtk4::TextView>,
    #[template_child]
    error_indicator: gtk4::TemplateChild<gtk4::Image>,

    /// Un-normalized range (min, value, max) of in value
    value_in: RefCell<(f32, f32, f32)>,

    /// Un-normalized range (min, value, max) of out value
    value_out: RefCell<(f32, f32, f32)>,

    #[property(name="binding-name", get=Self::binding_name, set=Self::set_binding_name)]
    #[property(name="expression", get=Self::expression, set=Self::set_expression)]
    #[property(name="expression-error", get=Self::expression_error, set=Self::set_expression_error)]
    _synths_string: RefCell<String>,

    #[property(name="dampen-level", get=Self::dampen_level, set=Self::set_dampen_level)]
    #[property(name="value-in-from", get=Self::value_in_from, set=Self::set_value_in_from)]
    #[property(name="value-in-to", get=Self::value_in_to, set=Self::set_value_in_to)]
    #[property(name="value-out-from", get=Self::value_out_from, set=Self::set_value_out_from)]
    #[property(name="value-out-to", get=Self::value_out_to, set=Self::set_value_out_to)]
    #[property(name="value-in", get=Self::value_in, set=Self::set_value_in)]
    #[property(name="value-out", get=Self::value_out, set=Self::set_value_out)]
    _synths_float: RefCell<f32>,

    #[property(name="inverse", get=Self::inverse, set=Self::set_inverse)]
    #[property(name="has-error", get=Self::has_error, set=Self::set_has_error)]
    _synths_bool: RefCell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for BindingFormImp {
    const NAME: &'static str = "NGTBindingForm";
    type Type = BindingForm;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

macro_rules! export_notify {
    ($self:ident, $inner_widget:ident, $inner_signal:ident, $outer_notify:ident) => {
        $self.$inner_widget.$inner_signal({
            let callback_self = $self.obj().clone();
            move |_| {
                callback_self.$outer_notify();
            }
        });
    };
}

#[glib::derived_properties]
impl ObjectImpl for BindingFormImp {
    fn constructed(&self) {
        self.parent_constructed();

        export_notify!(self, name, connect_label_notify, notify_binding_name);
        export_notify!(
            self,
            dampening_entry,
            connect_buffer_notify,
            notify_dampen_level
        );
        export_notify!(
            self,
            value_in_from_entry,
            connect_buffer_notify,
            notify_value_in_from
        );
        export_notify!(
            self,
            value_in_to_entry,
            connect_buffer_notify,
            notify_value_in_to
        );
        export_notify!(
            self,
            value_out_from_entry,
            connect_buffer_notify,
            notify_value_out_from
        );
        export_notify!(
            self,
            value_out_to_entry,
            connect_buffer_notify,
            notify_value_out_to
        );
        export_notify!(self, value_invert_check, connect_activate, notify_inverse);

        // TODO: These treat the GtkEntry as source of truth for in and out.
        // Can we do better?
        self.obj().connect_value_in_from_notify(|form| {
            let mut value = form.imp().value_in.borrow_mut();

            value.0 = form.value_in_from();
            form.imp()
                .update_level_bar(*value, &form.imp().value_in_display);
        });

        self.obj().connect_value_in_to_notify(|form| {
            let mut value = form.imp().value_in.borrow_mut();

            value.2 = form.value_in_to();
            form.imp()
                .update_level_bar(*value, &form.imp().value_in_display);
        });

        self.obj().connect_value_out_from_notify(|form| {
            let mut value = form.imp().value_out.borrow_mut();

            value.0 = form.value_out_from();
            form.imp()
                .update_level_bar(*value, &form.imp().value_out_display);
        });

        self.obj().connect_value_out_to_notify(|form| {
            let mut value = form.imp().value_out.borrow_mut();

            value.2 = form.value_out_to();
            form.imp()
                .update_level_bar(*value, &form.imp().value_out_display);
        });

        export_notify!(
            self,
            value_in_display,
            connect_value_notify,
            notify_value_in
        );
        export_notify!(
            self,
            value_out_display,
            connect_value_notify,
            notify_value_out
        );
        export_notify!(
            self,
            expression_entry,
            connect_buffer_notify,
            notify_expression
        );
        export_notify!(
            self,
            error_indicator,
            connect_visible_notify,
            notify_has_error
        );
    }
}

impl WidgetImpl for BindingFormImp {}

impl BoxImpl for BindingFormImp {}

macro_rules! float_property_impl {
    ($field_name:ident, $set_field_name:ident, $widget_name:ident) => {
        fn $field_name(&self) -> f32 {
            f32::from_str(self.$widget_name.buffer().text().as_str()).unwrap_or(f32::NAN)
        }

        fn $set_field_name(&self, value: f32) {
            self.$widget_name.buffer().set_text(format!("{}", value));
        }
    };
}

impl BindingFormImp {
    fn binding_name(&self) -> String {
        self.name.label().into()
    }

    fn set_binding_name(&self, value: String) {
        self.name.set_label(&value)
    }

    float_property_impl!(dampen_level, set_dampen_level, dampening_entry);
    float_property_impl!(value_in_from, set_value_in_from, value_in_from_entry);
    float_property_impl!(value_in_to, set_value_in_to, value_in_to_entry);
    float_property_impl!(value_out_from, set_value_out_from, value_out_from_entry);
    float_property_impl!(value_out_to, set_value_out_to, value_out_to_entry);

    fn inverse(&self) -> bool {
        self.value_invert_check.is_active()
    }

    fn set_inverse(&self, value: bool) {
        self.value_invert_check.set_active(value);
    }

    fn has_error(&self) -> bool {
        self.error_indicator.is_visible()
    }

    fn set_has_error(&self, value: bool) {
        self.error_indicator.set_visible(value);
    }

    fn value_in(&self) -> f32 {
        self.value_in.borrow().1
    }

    fn set_value_in(&self, value: f32) {
        self.value_in.borrow_mut().1 = value;
        self.update_level_bar(*self.value_in.borrow(), &self.value_in_display);
    }

    fn value_out(&self) -> f32 {
        self.value_out.borrow().1
    }

    fn set_value_out(&self, value: f32) {
        self.value_out.borrow_mut().1 = value;
        self.update_level_bar(*self.value_out.borrow(), &self.value_out_display);
    }

    fn expression(&self) -> String {
        self.expression_entry.buffer().text().into()
    }

    fn set_expression(&self, value: String) {
        self.expression_entry.buffer().set_text(value);
    }

    fn expression_error(&self) -> String {
        self.expression_error_label
            .buffer()
            .property::<String>("text")
    }

    fn set_expression_error(&self, value: String) {
        self.expression_error_label.buffer().set_text(&value);
    }

    /// Set the target level bar to display the range (min, value, max).
    ///
    /// This specifically normalizes the range to be 0 to (max - min), rejects
    /// NaNs, and flips the range around if it's inverted for whatever reason.
    fn update_level_bar(&self, range: (f32, f32, f32), target: &gtk4::LevelBar) {
        let (in_min, in_value, in_max) = range;

        // Do not attempt to update the level bar if the range has not yet been configured.
        if in_min.is_nan() || in_value.is_nan() || in_max.is_nan() {
            return;
        }

        let in_is_inverse = in_min > in_max;

        let normal_max = if in_is_inverse {
            in_min - in_max
        } else {
            in_max - in_min
        };

        let mut normal_value = if in_is_inverse {
            in_value - in_max
        } else {
            in_value - in_min
        };

        if normal_value < 0.0 {
            normal_value = 0.0;
        }

        target.set_min_value(0.0);
        target.set_max_value(normal_max as f64);
        target.set_value(normal_value as f64);
        target.set_inverted(in_is_inverse);
    }
}

glib::wrapper! {
    pub struct BindingForm(ObjectSubclass<BindingFormImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible, gtk4::Orientable;
}

impl BindingForm {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
