mod gtk_ext;
mod io_adapter;
mod json;
mod string_ext;

pub use crate::gtk_ext::GLAreaExt2;
pub use crate::gtk_ext::WidgetExt2;
pub use crate::io_adapter::FileIn;
pub use crate::json::{JsonIndex, JsonValueExt};
pub use crate::string_ext::StrExt;

pub mod prelude {
    pub use crate::gtk_ext::GLAreaExt2;
    pub use crate::gtk_ext::WidgetExt2;
    pub use crate::json::{JsonIndex, JsonValueExt};
    pub use crate::string_ext::StrExt;
}
