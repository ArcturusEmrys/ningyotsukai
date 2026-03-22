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
        drop_target.connect_drop(move |_, value, x, y| {
            let point = graphene::Point::new(x as f32, y as f32);
            if let Ok(frame) = value.get::<PanelFrame>() {
                if let Some(old_dock) = frame.parent() {
                    old_dock.downcast::<PanelDock>().unwrap().remove(&frame);
                }

                let mut my_child = drop_target_drop_self.first_child();
                let mut eligible_predecessor = None;
                while let Some(child) = my_child {
                    let child_rel = drop_target_drop_self.compute_point(&child, &point);
                    if let Some(child_rel) = child_rel {
                        if child_rel.y() >= 0.0 {
                            eligible_predecessor = Some(child.clone());
                        }
                    }

                    my_child = child.next_sibling();
                }

                if let Some(pre) = eligible_predecessor {
                    frame.insert_before(&drop_target_drop_self, Some(&pre));
                } else {
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
