use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;

use crate::panels::page_ref::PageRef;

#[derive(CompositeTemplate, Default, glib::Properties)]
#[template(resource = "/live/arcturus/ningyotsukai/panels/frame.ui")]
#[properties(wrapper_type=PanelFrame)]
pub struct PanelFrameImp {
    #[template_child]
    handles: TemplateChild<gtk4::Box>,

    #[template_child]
    contents: TemplateChild<gtk4::Stack>,

    #[property(set=Self::set_name)]
    name: RefCell<String>,
}

#[glib::object_subclass]
impl ObjectSubclass for PanelFrameImp {
    const NAME: &'static str = "NGTPanelFrame";
    type Type = PanelFrame;
    type ParentType = gtk4::Box;
    type Interfaces = (gtk4::Buildable,);

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.set_css_name("ningyo-panelframe");
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

#[glib::derived_properties]
impl ObjectImpl for PanelFrameImp {
    fn constructed(&self) {
        self.parent_constructed();

        *self.name.borrow_mut() = "UNINIT".to_string();

        self.populate_handles();

        let items_changed_self = self.obj().clone();
        self.contents
            .pages()
            .connect_items_changed(move |_, _, _, _| {
                items_changed_self.imp().populate_handles();
            });
    }
}

impl WidgetImpl for PanelFrameImp {}

impl BoxImpl for PanelFrameImp {}

impl BuildableImpl for PanelFrameImp {
    fn add_child(&self, builder: &gtk4::Builder, object: &glib::Object, name: Option<&str>) {
        if let Some(widget) = object.downcast_ref::<gtk4::Widget>() {
            match name {
                Some("NGTPanelFrame-internal") => self.parent_add_child(builder, object, name),
                _ => {
                    let count = self.contents.pages().n_items();
                    self.contents.add_titled(
                        widget,
                        Some(&format!("{}", count)),
                        self.name.borrow().as_str(),
                    );
                    self.populate_handles();
                }
            }
        } else {
            self.parent_add_child(builder, object, name)
        }
    }
}

impl PanelFrameImp {
    /// Set the name of the frame page.
    ///
    /// This is primarily intended to be used during XML construction, NOT the
    /// day-to-day functioning of your app. Notably, please do not bind this
    /// property.
    fn set_name(&self, new_name: String) {
        for page in self.contents.pages().iter::<gtk4::StackPage>() {
            let page = page.unwrap();
            page.set_title(&new_name);
        }

        *self.name.borrow_mut() = new_name;

        self.populate_handles();
    }

    fn populate_handles(&self) {
        while let Some(child) = self.handles.first_child() {
            child.unparent();
        }

        let mut has_selected_page = false;
        for page in self.contents.pages().iter::<gtk4::StackPage>() {
            if page.unwrap().is_visible() {
                has_selected_page = true;
            }
        }

        if !has_selected_page {
            if let Some(child) = self.contents.first_child() {
                self.contents.set_visible_child(&child);
            }
        }

        let mut group = None;

        for page in self.contents.pages().iter::<gtk4::StackPage>() {
            let page = page.unwrap();

            let is_visible = self.contents.visible_child() == Some(page.child());

            let label = gtk4::ToggleButton::builder()
                .label(page.title().unwrap())
                .active(is_visible)
                .css_classes(["NGTPanels-tab"])
                .build();
            label.set_group(group.as_ref());
            let drag_source = gtk4::DragSource::builder()
                .actions(gdk4::DragAction::MOVE)
                .propagation_phase(gtk4::PropagationPhase::Capture)
                .build();

            let drag_source_prepare_self = self.obj().clone();
            let drag_source_prepare_page = page.clone();
            drag_source.connect_prepare(move |_, _, _| {
                let value = glib::Value::from(PageRef {
                    frame: drag_source_prepare_self.clone(),
                    page: drag_source_prepare_page.clone(),
                });
                Some(gdk4::ContentProvider::for_value(&value))
            });

            let drag_source_begin_self = self.obj().clone();
            drag_source.connect_drag_begin(move |source, _| {
                let preview = gtk4::WidgetPaintable::new(Some(&drag_source_begin_self));
                source.set_icon(Some(&preview.current_image()), 0, 0);
            });

            let drag_source_end_self = self.obj().clone();
            drag_source.connect_drag_end(move |_, _, _| {
                drag_source_end_self.imp().populate_handles();
            });

            label.add_controller(drag_source);

            let toggle_self = self.obj().clone();
            let toggle_page = page.clone();
            label.connect_clicked(move |_| {
                toggle_self
                    .imp()
                    .contents
                    .set_visible_child(&toggle_page.child())
            });

            self.handles.append(&label);

            if group.is_none() {
                group = Some(label.clone());
            }
        }
    }
}

glib::wrapper! {
    pub struct PanelFrame(ObjectSubclass<PanelFrameImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl PanelFrame {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn n_pages(&self) -> u32 {
        self.imp().contents.pages().n_items()
    }

    /// Given a panel frame, merge its pages into this widget.
    pub fn adopt_page(&self, from_panel: PanelFrame, incoming_page: gtk4::StackPage) {
        let widget = incoming_page.child();
        let title = incoming_page.title();

        from_panel.imp().contents.remove(&widget);
        self.imp()
            .contents
            .add_titled(&widget, None, title.as_deref().unwrap_or(""));
        self.imp().populate_handles();
    }
}
