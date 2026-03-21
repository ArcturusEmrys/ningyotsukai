#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod generic;

pub fn init() {
    gio::resources_register_include!("resources.gresource").expect("valid resource file");

    #[cfg(windows)]
    windows::init();

    #[cfg(not(windows))]
    generic::init();
}
