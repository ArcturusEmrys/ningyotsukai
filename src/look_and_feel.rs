#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod generic;

pub fn init() {
    #[cfg(windows)]
    windows::init();

    #[cfg(not(windows))]
    generic::init();
}
