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

    #[property(name="binding-name", get=Self::binding_name, set=Self::set_binding_name)]
    _synths_string: RefCell<String>,

    #[property(name="dampen-level", get=Self::dampen_level, set=Self::set_dampen_level)]
    #[property(name="value-in-from", get=Self::value_in_from, set=Self::set_value_in_from)]
    #[property(name="value-in-to", get=Self::value_in_to, set=Self::set_value_in_to)]
    #[property(name="value-out-from", get=Self::value_out_from, set=Self::set_value_out_from)]
    #[property(name="value-out-to", get=Self::value_out_to, set=Self::set_value_out_to)]
    _synths_float: RefCell<f32>,

    #[property(name="inverse", get=Self::inverse, set=Self::set_inverse)]
    _synths_bool: RefCell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for BindingFormImp {
    const NAME: &'static str = "NGTBindingForm";
    type Type = BindingForm;
    type ParentType = gtk4::Grid;

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
    }
}

impl WidgetImpl for BindingFormImp {}

impl GridImpl for BindingFormImp {}

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
}

glib::wrapper! {
    pub struct BindingForm(ObjectSubclass<BindingFormImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible, gtk4::Orientable;
}

impl BindingForm {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
