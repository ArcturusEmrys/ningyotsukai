use gio::prelude::*;
use gtk4::prelude::*;

use ningyo_look_and_feel;

mod document;
mod io;
mod panels;
mod stage;
mod tracker;
mod window;

fn main() -> glib::ExitCode {
    //io_send.send_blocking(io::IoMessage::ConnectVTSTracker())

    gio::resources_register_include!("resources.gresource").expect("valid resource file");
    gtk4::init().expect("valid gtk4 state");

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

    app.connect_activate({
        move |app| {
            let tracker_manager: std::rc::Rc<tracker::TrackerManager> =
                tracker::TrackerManager::new();

            panels::PanelDock::ensure_type();
            panels::PanelFrame::ensure_type();

            let window = window::WindowController::new(app, tracker_manager.clone());

            window.present();
        }
    });

    app.run()
}
