use gio::prelude::*;
use glib::subclass::{InitializingObject, Signal, SignalType};
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use ningyo_extensions::prelude::*;

use crate::tracker::model::{Tracker, TrackerType};

use std::sync::OnceLock;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/tracker/form.ui")]
pub struct TrackerFormImp {
    #[template_child]
    ok_button: gtk4::TemplateChild<gtk4::Button>,
    #[template_child]
    cancel_button: gtk4::TemplateChild<gtk4::Button>,

    #[template_child]
    name_field: gtk4::TemplateChild<gtk4::Entry>,
    #[template_child]
    ip_addr_field: gtk4::TemplateChild<gtk4::Entry>,
}

#[glib::object_subclass]
impl ObjectSubclass for TrackerFormImp {
    const NAME: &'static str = "NGTTrackerForm";
    type Type = TrackerForm;
    type ParentType = gtk4::Window;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for TrackerFormImp {
    fn constructed(&self) {
        self.parent_constructed();

        let ok_clicked_self = self.obj().clone();
        self.ok_button.connect_clicked(move |_| {
            ok_clicked_self.emit_save(Tracker {
                name: ok_clicked_self.imp().name_field.buffer().text().to_string(),
                tracker_type: TrackerType::VTS(
                    ok_clicked_self
                        .imp()
                        .ip_addr_field
                        .buffer()
                        .text()
                        .to_string(),
                ),
            });
        });

        let cancel_clicked_self = self.obj().clone();
        self.cancel_button.connect_clicked(move |_| {
            cancel_clicked_self.emit_cancel();
        });
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                glib::subclass::Signal::builder("save")
                    .param_types([SignalType::with_static_scope(Tracker::static_type())])
                    .build(),
                glib::subclass::Signal::builder("cancel").build(),
            ]
        })
    }
}

impl WidgetImpl for TrackerFormImp {}

impl WindowImpl for TrackerFormImp {}

glib::wrapper! {
    pub struct TrackerForm(ObjectSubclass<TrackerFormImp>)
        @extends gtk4::Window, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible, gtk4::ShortcutManager, gtk4::Root, gtk4::Native;
}

impl TrackerForm {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn populate_with_tracker(&self, tracker: &Tracker) {
        self.imp()
            .name_field
            .buffer()
            .set_text(&*tracker.name.escape_nulls());

        match &tracker.tracker_type {
            TrackerType::VTS(ip_addr) => {
                self.imp()
                    .ip_addr_field
                    .buffer()
                    .set_text(&*ip_addr.escape_nulls());
            }
        }
    }
}

pub trait TrackerFormExt {
    fn connect_save<F: Fn(&Self, Tracker) + 'static>(&self, f: F) -> glib::SignalHandlerId;

    fn emit_save(&self, tracker: Tracker);

    fn connect_cancel<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId;

    fn emit_cancel(&self);
}

impl<T: IsA<TrackerForm>> TrackerFormExt for T {
    fn connect_save<F: Fn(&Self, Tracker) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("save", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            let tracker = values[1].get::<Tracker>().unwrap();
            f(&me, tracker);
            None
        })
    }

    fn emit_save(&self, tracker: Tracker) {
        self.emit_by_name::<()>("save", &[&tracker]);
    }

    fn connect_cancel<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("cancel", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            f(&me);
            None
        })
    }

    fn emit_cancel(&self) {
        self.emit_by_name::<()>("cancel", &[]);
    }
}
