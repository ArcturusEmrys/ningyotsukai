use gio;
use glib;
use gtk4;
use gtk4::prelude::*;

mod detail_views;
mod document;
mod io_adapter;
mod navigation_item;
mod string_ext;
mod window;

use crate::window::WindowController;

fn main() -> glib::ExitCode {
    gio::resources_register_include!("resources.gresource").expect("valid resource file");

    let app = gtk4::Application::builder()
        .application_id("live.arcturus.puppet-inspector")
        .build();

    app.connect_activate(|app| {
        let window = WindowController::new(app);

        window.present();
    });

    app.run()
}
