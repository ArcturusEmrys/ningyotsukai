pub use crate::binding::Binding;
use inox2d::model::VendorData;

const VENDOR_KEY: &str = "com.inochi2d.inochi-session.bindings";

mod binding;
pub mod tracker;
pub mod vts;

pub fn parse_bindings(vendor_data: &[VendorData]) -> Option<Vec<Binding>> {
    for vendor in vendor_data {
        if vendor.name == VENDOR_KEY {
            return Binding::from_payload(&vendor.payload);
        }
    }

    None
}
