use gio;
use glib;
use gtk4;
use gtk4::prelude::*;

mod detail_views;
mod document;
mod ext;
mod navigation;
mod render_preview;
mod window;

use crate::window::WindowController;
use ningyo_look_and_feel;

fn main() -> glib::ExitCode {
    gio::resources_register_include!("resources.gresource").expect("valid resource file");
    gtk4::init().expect("valid gtk4 state");

    ningyo_look_and_feel::init();

    let laf_css = gtk4::CssProvider::new();
    laf_css.load_from_resource("/live/arcturus/puppet-inspector/style.css");

    let display = gdk4::Display::default().expect("display");
    gtk4::style_context_add_provider_for_display(
        &display,
        &laf_css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let app = gtk4::Application::builder()
        .application_id("live.arcturus.puppet-inspector")
        .build();

    app.connect_activate(|app| {
        let window = WindowController::new(app);

        window.present();
    });

    app.run()
}
