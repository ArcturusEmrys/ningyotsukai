mod error;
mod texshare;
mod widget;

pub use error::Error;
pub use texshare::linux::ExportedTexture;
pub use widget::WgpuArea;

pub mod prelude {
    pub use crate::texshare::{AdapterExt, DeviceExt, InstanceExt};
    pub use crate::widget::WgpuAreaExt;
}

pub mod subclass {
    pub mod prelude {
        pub use crate::widget::WgpuAreaImpl;
    }
}
