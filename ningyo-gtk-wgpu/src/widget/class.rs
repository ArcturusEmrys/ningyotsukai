//! GtkGlArea analog that provides a drawable WGPU surface.

use std::cell::RefCell;
use std::sync::OnceLock;

use glib;
use gtk4;

use glib::subclass::signal::SignalType;
use glib::subclass::{InitializingObject, Signal};
use glib::types::StaticType;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::widget::subclass::{WgpuAreaClass, WgpuAreaImpl};

#[derive(Default)]
struct WgpuAreaState {
    needs_resize: bool,
    needs_render: bool,
    texture: Option<gdk4::Texture>,
}

#[derive(Default)]
pub struct WgpuAreaImp {
    state: RefCell<WgpuAreaState>,
}

#[glib::object_subclass]
impl ObjectSubclass for WgpuAreaImp {
    const NAME: &'static str = "NGWgpuArea";
    type Type = WgpuArea;
    type ParentType = gtk4::Widget;
    type Class = WgpuAreaClass;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-wgpuarea");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for WgpuAreaImp {
    fn constructed(&self) {
        self.parent_constructed();
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("render")
                    .return_type::<bool>()
                    .run_last()
                    .class_handler(|args| {
                        let obj = args[0].get::<Self::Type>().unwrap();
                        if let Some(render) = obj.class().as_ref().render {
                            Some(unsafe {
                                render(obj.as_ptr() as *mut glib::gobject_ffi::GObject).into()
                            })
                        } else {
                            None
                        }
                    })
                    .accumulator(|_hint, _accum, retval| {
                        let handled: bool = retval.get().unwrap();

                        if handled {
                            std::ops::ControlFlow::Break(handled.into())
                        } else {
                            std::ops::ControlFlow::Continue(handled.into())
                        }
                    })
                    .build(),
                Signal::builder("resize")
                    .param_types([
                        SignalType::with_static_scope(i32::static_type()),
                        SignalType::with_static_scope(i32::static_type()),
                    ])
                    .return_type::<bool>()
                    .run_last()
                    .class_handler(|args| {
                        let obj = args[0].get::<Self::Type>().unwrap();
                        let x = args[1].get::<i32>().unwrap();
                        let y = args[2].get::<i32>().unwrap();
                        if let Some(resize) = obj.class().as_ref().resize {
                            Some(unsafe {
                                resize(obj.as_ptr() as *mut glib::gobject_ffi::GObject, x, y).into()
                            })
                        } else {
                            None
                        }
                    })
                    .build(),
            ]
        })
    }
}

impl WidgetImpl for WgpuAreaImp {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        self.state.borrow_mut().needs_resize = true;
        self.obj().queue_draw();
    }

    fn snapshot(&self, snapshot: &gtk4::Snapshot) {
        let needs_resize = self.state.borrow().needs_resize;
        let needs_render = self.state.borrow().needs_render;

        if needs_render {
            if needs_resize {
                self.obj()
                    .emit_resize(self.obj().width(), self.obj().height());
                self.state.borrow_mut().needs_resize = false;
            }

            self.obj().emit_render();
        }

        if let Some(ref texture) = self.state.borrow().texture {
            dbg!(texture);
            snapshot.append_texture(
                texture,
                &graphene::Rect::new(
                    0.0,
                    0.0,
                    self.obj().width() as f32,
                    self.obj().height() as f32,
                ),
            );

            eprintln!("DONE");
        }
    }
}

impl WgpuAreaImpl for WgpuAreaImp {
    fn render(&self) -> glib::Propagation {
        glib::Propagation::Stop
    }

    fn resize(&self, _x: i32, _y: i32) -> glib::Propagation {
        glib::Propagation::Stop
    }
}

glib::wrapper! {
    pub struct WgpuArea(ObjectSubclass<WgpuAreaImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl WgpuArea {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Attach a texture to this widget.
    ///
    /// The texture should be the same size as the widget and contain the
    /// image you want to render. You can use the texture sharing
    /// infrastructure to provide a texture to this widget that you will then
    /// render to with your wgpu instance.
    pub fn attach_texture(&self, texture: gdk4::Texture) {
        self.imp().state.borrow_mut().texture = Some(texture);
        self.queue_render();
    }

    pub fn queue_render(&self) {
        self.imp().state.borrow_mut().needs_render = true;
        self.queue_draw();
    }
}

pub trait WgpuAreaExt {
    /// Signal fired to indicate that the drawing area has been resized and
    /// the widget should allocate and provide a new texture to render.
    fn connect_resize<F: Fn(&Self, i32, i32) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId;

    fn emit_resize(&self, w: i32, h: i32);

    /// Signal fired to indicate that the widget is rendering now and that the
    /// associated texture should be updated or replaced.
    fn connect_render<F: Fn(&Self) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId;

    fn emit_render(&self);

    fn attach_texture(&self, texture: gdk4::Texture);

    fn queue_render(&self);
}

impl<T> WgpuAreaExt for T
where
    T: IsA<WgpuArea>,
{
    fn connect_resize<F: Fn(&Self, i32, i32) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("resize", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            let w = values[1].get::<i32>().unwrap();
            let h = values[2].get::<i32>().unwrap();
            Some(f(&me, w, h).into())
        })
    }

    fn emit_resize(&self, w: i32, h: i32) {
        self.emit_by_name::<bool>("resize", &[&w, &h]);
    }

    fn connect_render<F: Fn(&Self) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("render", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            Some(f(&me).into())
        })
    }

    fn emit_render(&self) {
        self.emit_by_name::<bool>("render", &[]);
    }

    fn attach_texture(&self, texture: gdk4::Texture) {
        self.clone().upcast::<WgpuArea>().attach_texture(texture)
    }

    fn queue_render(&self) {
        self.clone().upcast::<WgpuArea>().queue_render()
    }
}
