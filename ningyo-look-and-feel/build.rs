use glib_build_tools;

fn main() {
    glib_build_tools::compile_resources(
        &["src"],
        "src/resources.gresource.xml",
        "resources.gresource",
    );
}
