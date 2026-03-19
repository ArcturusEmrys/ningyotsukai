pub fn init() {
    let laf_css = gtk4::CssProvider::new();
    laf_css.load_from_resource("/live/arcturus/puppet-inspector/look-and-feel.css");

    let display = gdk4::Display::default().expect("display");
    gtk4::style_context_add_provider_for_display(
        &display,
        &laf_css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
