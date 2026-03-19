use glib_build_tools;

fn main() {
    glib_build_tools::compile_resources(
        &["resources", "src"],
        "resources/resources.gresource.xml",
        "resources.gresource",
    );
}
