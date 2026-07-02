#![windows_subsystem = "windows"]

use gio::prelude::*;
use gtk4::prelude::*;

mod artboard;
mod bindings;
mod document;
mod inspector;
mod io;
mod panels;
mod render;
mod stage;
mod tracker;
mod window;

fn main() -> glib::ExitCode {
    env_logger::init();

    gio::resources_register_include!("resources.gresource").expect("valid resource file");
    gtk4::init().expect("valid gtk4 state");

    #[cfg(feature = "branding")]
    ningyo_look_and_feel::init();

    let laf_css = gtk4::CssProvider::new();
    laf_css.load_from_resource("/live/arcturus/ningyotsukai/style.css");

    let display = gdk4::Display::default().expect("display");
    gtk4::style_context_add_provider_for_display(
        &display,
        &laf_css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let app = gtk4::Application::builder()
        .application_id("live.arcturus.ningyotsukai")
        .build();

    let document_manager = document::DocumentManager::new();

    app.connect_activate({
        let my_document_manager = document_manager.clone();
        move |app| {
            let tracker_manager: std::rc::Rc<tracker::TrackerManager> =
                tracker::TrackerManager::new();

            panels::PanelDock::ensure_type();
            panels::PanelFrame::ensure_type();
            tracker::TrackerPanel::ensure_type();
            tracker::TrackerParamPanel::ensure_type();
            bindings::BindingPanel::ensure_type();
            artboard::ArtboardPanel::ensure_type();
            inspector::InspectorPanel::ensure_type();

            let window = window::WindowController::new(
                app,
                tracker_manager.clone(),
                my_document_manager.clone(),
            );

            window.present();
        }
    });

    app.connect_shutdown(move |_| {
        document_manager.clone().shutdown();
    });

    app.run()
}
