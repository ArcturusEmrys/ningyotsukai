use gdk4;
use glib;
use graphene;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::panels::PanelFrame;

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/ningyotsukai/panels/dock.ui")]
pub struct PanelDockImp {}

#[glib::object_subclass]
impl ObjectSubclass for PanelDockImp {
    const NAME: &'static str = "NGTPanelDock";
    type Type = PanelDock;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.set_css_name("ningyo-paneldock");
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for PanelDockImp {
    fn constructed(&self) {
        self.parent_constructed();

        let drop_target = gtk4::DropTarget::new(PanelFrame::static_type(), gdk4::DragAction::COPY);
        let drop_target_drop_self = self.obj().clone();
        drop_target.connect_drop(move |_, value, _x, y| {
            if let Ok(frame) = value.get::<PanelFrame>() {
                let mut my_child = drop_target_drop_self.first_child();
                let mut index = 0;
                let mut drop_target_widget = None;
                let mut frame_child_index = None;
                while let Some(child) = my_child {
                    let bounds = child.compute_bounds(&drop_target_drop_self);
                    if let Some(bounds) = bounds {
                        if bounds.y() <= y as f32 && (y as f32) < bounds.y() + bounds.height() {
                            drop_target_widget = Some((child.clone(), index));
                        }
                    }

                    if child == frame.clone().upcast::<gtk4::Widget>() {
                        frame_child_index = Some(index);
                    }

                    my_child = child.next_sibling();
                    index += 1;
                }

                if let Some((pre, pre_index)) = drop_target_widget {
                    if pre != frame.clone().upcast::<gtk4::Widget>() {
                        frame.unparent();

                        if let Some(frame_index) = frame_child_index
                            && frame_index < pre_index
                        {
                            frame.insert_after(&drop_target_drop_self, Some(&pre));
                        } else {
                            frame.insert_before(&drop_target_drop_self, Some(&pre));
                        }
                    }
                } else {
                    frame.unparent();
                    drop_target_drop_self.append(&frame);
                }
                return true;
            }

            false
        });

        self.obj().add_controller(drop_target);
    }
}

impl WidgetImpl for PanelDockImp {}

impl BoxImpl for PanelDockImp {}

glib::wrapper! {
    pub struct PanelDock(ObjectSubclass<PanelDockImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl PanelDock {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}
