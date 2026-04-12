mod widget;

pub use widget::WgpuArea;

pub mod prelude {
    pub use crate::widget::WgpuAreaExt;
}

pub mod subclass {
    pub mod prelude {
        pub use crate::widget::WgpuAreaImpl;
    }
}
