use glib::WeakRef;
use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use ningyo_binding::{Binding, BindingType};
use ningyo_extensions::prelude::*;

use crate::bindings::form::BindingForm;
use crate::document::Document;
use crate::stage::{StageWidget, StageWidgetExt};
use generational_arena::Index;

use std::cell::RefCell;

struct State {
    document: Document,

    stage: WeakRef<StageWidget>,

    current_selection: Option<Index>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/bindings/panel.ui")]
pub struct BindingPanelImp {
    state: RefCell<Option<State>>,

    #[template_child]
    bindings_contents: gtk4::TemplateChild<gtk4::Box>,
}

#[glib::object_subclass]
impl ObjectSubclass for BindingPanelImp {
    const NAME: &'static str = "NGTBindingPanel";
    type Type = BindingPanel;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for BindingPanelImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for BindingPanelImp {}

impl BoxImpl for BindingPanelImp {}

impl BindingPanelImp {
    fn with_binding_mut<F: FnOnce(&mut Binding)>(&self, binding_index: usize, f: F) {
        let mut state = self.state.borrow_mut();
        let state = state.as_mut().unwrap();

        if let Some(select) = state.current_selection {
            if let Some(mut puppet) = state.document.stage_mut().puppet(select) {
                if let Some((binding, _in, _out, _error)) =
                    puppet.bindings_mut().get_mut(binding_index)
                {
                    f(binding)
                }
            }
        }
    }

    fn populate_list(&self) {
        let mut state = self.state.borrow_mut();
        let state = state.as_mut().unwrap();

        // We make no attempt at showing multiple selected puppets' bindings
        // at once. We probably should treat this as "no selection" and clear
        // everything.
        let stage = state.stage.upgrade().unwrap();
        let new_selection = stage.selection().iter().next().copied();
        if new_selection == state.current_selection {
            // We can get spurious selection notifications, in which case do
            // nothing.
            return;
        }

        state.current_selection = new_selection;

        while let Some(child) = self.bindings_contents.first_child() {
            self.bindings_contents.remove(&child);
        }

        if let Some(index) = new_selection {
            if let Some(puppet) = state.document.stage().puppet(index) {
                for (binding_index, (binding, in_value, out_value, last_error)) in
                    puppet.bindings().iter().enumerate()
                {
                    let form = BindingForm::new();

                    form.set_binding_name(binding.name.escape_nulls());
                    form.set_dampen_level(binding.dampen_level);

                    match &binding.binding_type {
                        BindingType::Ratio(ratio) => {
                            form.set_value_in_from(ratio.in_range.x);
                            form.set_value_in_to(ratio.in_range.y);
                            form.set_value_out_from(ratio.out_range.x);
                            form.set_value_out_to(ratio.out_range.y);
                            form.set_inverse(ratio.inverse);
                        }
                        BindingType::Expression(expr) => {
                            form.set_expression(expr.as_str());

                            if let Some(last_error) = last_error {
                                form.set_expression_error(last_error.to_string());
                                form.set_has_error(true);
                            } else {
                                form.set_has_error(false);
                            }
                        }
                    }

                    form.set_value_in(in_value);
                    form.set_value_out(out_value);

                    macro_rules! bind_float_property {
                        ($notify_signal:ident, $form_prop:ident, $binding_index:ident, |$value:ident, $binding:ident| $code:block) => {
                            form.$notify_signal({
                                let callback_self = self.obj().clone().downgrade();
                                move |form| {
                                    if let Some(callback_self) = callback_self.upgrade() {
                                        let $value = form.$form_prop();

                                        // NAN indicates a non-float value (user is still typing)
                                        if !$value.is_nan() {
                                            callback_self
                                                .imp()
                                                .with_binding_mut($binding_index, |$binding| $code);
                                        }
                                    }
                                }
                            });
                        };
                    }

                    bind_float_property!(
                        connect_dampen_level_notify,
                        dampen_level,
                        binding_index,
                        |value, binding| {
                            binding.dampen_level = value;
                        }
                    );

                    //NOTE: We deliberately bind both range and expression
                    //params in case the user changes modes.
                    bind_float_property!(
                        connect_value_in_from_notify,
                        value_in_from,
                        binding_index,
                        |value, binding| {
                            if let BindingType::Ratio(ratio) = &mut binding.binding_type {
                                ratio.in_range.x = value;
                            }
                        }
                    );

                    bind_float_property!(
                        connect_value_in_to_notify,
                        value_in_to,
                        binding_index,
                        |value, binding| {
                            if let BindingType::Ratio(ratio) = &mut binding.binding_type {
                                ratio.in_range.y = value;
                            }
                        }
                    );

                    bind_float_property!(
                        connect_value_out_from_notify,
                        value_out_from,
                        binding_index,
                        |value, binding| {
                            if let BindingType::Ratio(ratio) = &mut binding.binding_type {
                                ratio.out_range.x = value;
                            }
                        }
                    );

                    bind_float_property!(
                        connect_value_out_to_notify,
                        value_out_to,
                        binding_index,
                        |value, binding| {
                            if let BindingType::Ratio(ratio) = &mut binding.binding_type {
                                ratio.out_range.y = value;
                            }
                        }
                    );

                    form.connect_inverse_notify({
                        let callback_self = self.obj().clone().downgrade();
                        move |form| {
                            if let Some(callback_self) = callback_self.upgrade() {
                                let value = form.inverse();
                                callback_self
                                    .imp()
                                    .with_binding_mut(binding_index, |binding| {
                                        if let BindingType::Ratio(ratio) = &mut binding.binding_type
                                        {
                                            ratio.inverse = value;
                                        }
                                    });
                            }
                        }
                    });

                    form.connect_expression_notify({
                        let callback_self = self.obj().clone().downgrade();
                        move |form| {
                            if let Some(callback_self) = callback_self.upgrade() {
                                let value = form.expression();
                                callback_self
                                    .imp()
                                    .with_binding_mut(binding_index, |binding| {
                                        if let BindingType::Expression(expr) =
                                            &mut binding.binding_type
                                        {
                                            *expr = value;
                                        }
                                    });
                            }
                        }
                    });

                    self.bindings_contents.append(&form);
                }
            }
        }
    }

    /// Update all of the forms currently displayed.
    fn update_forms(&self) {
        let state = self.state.borrow();
        let state = state.as_ref().unwrap();

        if let Some(selection) = state.current_selection {
            let puppet = state.document.stage().puppet(selection).unwrap();

            let mut maybe_form = self.bindings_contents.first_child();
            let mut binding_index = 0;
            while let Some(widget) = maybe_form {
                if let Some(form) = widget.downcast_ref::<BindingForm>() {
                    form.set_value_in(puppet.bindings()[binding_index].1);
                    form.set_value_out(puppet.bindings()[binding_index].2);

                    if let Some(last_error) = &puppet.bindings()[binding_index].3 {
                        form.set_expression_error(last_error.to_string());
                        form.set_has_error(true);
                    } else {
                        form.set_has_error(false);
                    }

                    binding_index += 1;
                }

                maybe_form = widget.next_sibling();
            }
        }
    }
}

glib::wrapper! {
    pub struct BindingPanel(ObjectSubclass<BindingPanelImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl BindingPanel {
    pub fn bind(&self, document: Document, stage: StageWidget) {
        *self.imp().state.borrow_mut() = Some(State {
            document,
            stage: stage.downgrade(),
            current_selection: None,
        });

        stage.connect_selection_changed({
            let callback_self = self.clone().downgrade();
            move |_| {
                if let Some(callback_self) = callback_self.upgrade() {
                    callback_self.imp().populate_list();
                }
            }
        });

        stage.connect_updated({
            let callback_self = self.clone().downgrade();
            move |_| {
                if let Some(callback_self) = callback_self.upgrade() {
                    callback_self.imp().update_forms();
                }
            }
        });

        self.imp().populate_list();
    }
}
