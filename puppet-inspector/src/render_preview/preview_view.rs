use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use glam::{Vec4, Vec4Swizzles};

use gtk4::subclass::prelude::*;
use gtk4::{gsk, prelude::*};
use inox2d::math::rect::RectBounds;
use inox2d::node::components::{Mesh, TransformStore};

use crate::document::Document;
use crate::render_preview::debug_bounds::DebugBounds;
use crate::render_preview::debug_highlight::DebugHighlight;
use crate::render_preview::opengl::InoxGLPreview;
use crate::render_preview::wgpu::InoxWgpuPreview;
use inox2d::node::InoxNodeUuid;

#[derive(Default)]
pub struct PreviewViewState {
    current_renderer: Option<gtk4::Widget>,

    debug_highlight: Option<(
        Arc<Mutex<Document>>,
        InoxNodeUuid,
        DebugHighlight,
        DebugBounds,
    )>,
}

#[derive(Default)]
pub struct PreviewViewImp {
    state: RefCell<PreviewViewState>,
}

#[glib::object_subclass]
impl ObjectSubclass for PreviewViewImp {
    const NAME: &'static str = "PIPreviewView";
    type Type = PreviewView;
    type ParentType = gtk4::Widget;
}

impl ObjectImpl for PreviewViewImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for PreviewViewImp {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        if let Some(renderer) = self.state.borrow_mut().current_renderer.as_mut() {
            renderer.size_allocate(&gtk4::Allocation::new(0, 0, width, height), baseline);
        }

        self.parent_size_allocate(width, height, baseline);
    }
}

glib::wrapper! {
    pub struct PreviewView(ObjectSubclass<PreviewViewImp>)
        @extends gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget,
            gtk4::Accessible;
}

impl PreviewView {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn use_renderer(&self, renderer_widget: &gtk4::Widget) {
        if let Some(old_renderer) = self.imp().state.borrow_mut().current_renderer.take() {
            old_renderer.unparent();
        }

        renderer_widget.insert_before(self, self.first_child().as_ref());
        renderer_widget.measure(gtk4::Orientation::Horizontal, -1);
        renderer_widget.allocate(self.width(), self.height(), -1, None);

        self.imp().state.borrow_mut().current_renderer = Some(renderer_widget.clone());
    }

    /// Add a debug highlight for a particular node.
    pub fn enable_debug_highlight(&self, document: Arc<Mutex<Document>>, node: InoxNodeUuid) {
        if let Some((_doc, _node, dh, db)) = self.imp().state.borrow_mut().debug_highlight.take() {
            dh.unparent();
            db.unparent();
        }

        let db = DebugBounds::new();
        db.set_parent(self);

        let dh = DebugHighlight::new();
        dh.set_parent(self);

        self.imp().state.borrow_mut().debug_highlight = Some((document, node, dh, db));
        self.did_update();
    }

    pub fn disable_debug_highlight(&self) {
        if let Some((_doc, _node, dh, db)) = self.imp().state.borrow_mut().debug_highlight.take() {
            dh.unparent();
            db.unparent();
        }
    }

    /// Called by child renderers to tell the window that the puppet updated.
    pub fn did_update(&self) {
        let state = self.imp().state.borrow();

        if let Some((document, node_uuid, dh, db)) = state.debug_highlight.as_ref() {
            let document = document.lock().unwrap();

            if let Some(ts) = document
                .model
                .puppet
                .world()
                .get::<inox2d::node::components::TransformStore>(*node_uuid)
            {
                let origin = ts.absolute.mul_vec4(Vec4::new(0.0, 0.0, 0.0, 1.0)).xy();
                let bounds = {
                    let mut out = None;
                    for other_node in document.model.puppet.nodes().iter() {
                        let mut is_child_we_care_about = other_node.uuid == *node_uuid;
                        let mut ancestor = other_node.uuid;
                        while !is_child_we_care_about {
                            is_child_we_care_about |= ancestor == *node_uuid;

                            if ancestor != document.model.puppet.nodes().root_node_id {
                                ancestor = document.model.puppet.nodes().get_parent(ancestor).uuid;
                            } else {
                                break;
                            }
                        }

                        if !is_child_we_care_about {
                            continue;
                        }

                        if let (Some(transform), Some(mesh)) = (
                            document
                                .model
                                .puppet
                                .world()
                                .get::<TransformStore>(other_node.uuid),
                            document.model.puppet.world().get::<Mesh>(other_node.uuid),
                        ) {
                            let mvp = transform.absolute;

                            for vert in &mesh.vertices {
                                let vert =
                                    mvp.mul_vec4(glam::Vec4::new(vert.x, vert.y, 0.0, 1.0)).xy();
                                out = match out {
                                    None => Some(RectBounds::from_point(vert)),
                                    Some(rect) => Some(rect.with_union_point(vert)),
                                };
                            }
                        }
                    }

                    out
                }
                .unwrap_or_else(|| RectBounds::from_point(origin));

                drop(document);

                // TODO: This should probably be a GTK interface.
                let outer = if let Some(wgpu_preview) = state
                    .current_renderer
                    .clone()
                    .and_then(|r| r.downcast::<InoxWgpuPreview>().ok())
                {
                    wgpu_preview.puppet_to_widget(origin)
                } else if let Some(ogl_preview) = state
                    .current_renderer
                    .clone()
                    .and_then(|r| r.downcast::<InoxGLPreview>().ok())
                {
                    ogl_preview.puppet_to_widget(origin)
                } else {
                    return;
                };

                let mode = dh.request_mode();
                let (_, width, _, width_base) = dh.measure(gtk4::Orientation::Horizontal, -1);
                let (_, height, _, height_base) = dh.measure(gtk4::Orientation::Vertical, -1);
                let x = outer.x - (width as f32 / 2.0);
                let y = outer.y - (height as f32 / 2.0);
                let baseline = match mode {
                    gtk4::SizeRequestMode::WidthForHeight => width_base,
                    gtk4::SizeRequestMode::HeightForWidth | _ => height_base,
                };
                dh.allocate(
                    width,
                    height,
                    baseline,
                    Some(gsk::Transform::new().translate(&graphene::Point::new(x, y))),
                );

                //TODO: ibid.
                let (top_left, bottom_right) = if let Some(wgpu_preview) = state
                    .current_renderer
                    .clone()
                    .and_then(|r| r.downcast::<InoxWgpuPreview>().ok())
                {
                    (
                        wgpu_preview.puppet_to_widget(bounds.top_left_point()),
                        wgpu_preview.puppet_to_widget(bounds.bottom_right_point()),
                    )
                } else if let Some(ogl_preview) = state
                    .current_renderer
                    .clone()
                    .and_then(|r| r.downcast::<InoxGLPreview>().ok())
                {
                    (
                        ogl_preview.puppet_to_widget(bounds.top_left_point()),
                        ogl_preview.puppet_to_widget(bounds.bottom_right_point()),
                    )
                } else {
                    return;
                };

                let mode = db.request_mode();
                let (_, _, _, width_base) =
                    db.measure(gtk4::Orientation::Horizontal, bounds.width() as i32);
                let (_, _, _, height_base) =
                    db.measure(gtk4::Orientation::Vertical, bounds.height() as i32);
                let x = top_left.x;
                let y = top_left.y;
                let baseline = match mode {
                    gtk4::SizeRequestMode::WidthForHeight => width_base,
                    gtk4::SizeRequestMode::HeightForWidth | _ => height_base,
                };
                db.allocate(
                    (bottom_right.x - top_left.x) as i32,
                    (bottom_right.y - top_left.y) as i32,
                    baseline,
                    Some(gsk::Transform::new().translate(&graphene::Point::new(x, y))),
                );
            }
        }
    }
}
