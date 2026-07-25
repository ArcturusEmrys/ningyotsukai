use glib_build_tools;

#[cfg(all(target_os = "windows", feature = "branding"))]
fn add_windows_icon() {
    let mut app_ico = ico::IconDir::new(ico::ResourceType::Icon);

    for path in &[
        "branding/Puppet Inspector Icon 16.png",
        "branding/Puppet Inspector Icon 32.png",
        "branding/Puppet Inspector Icon 64.png",
        "branding/Puppet Inspector Icon 128.png",
    ] {
        println!("cargo:rerun-if-changed={}", path);
        let file = std::fs::File::open(path).unwrap();
        let image = ico::IconImage::read_png(file).unwrap();
        app_ico.add_entry(ico::IconDirEntry::encode(&image).unwrap());
    }

    std::fs::create_dir_all("build").unwrap();

    let ico_out = std::fs::File::create("build/Puppet Inspector.ico").unwrap();
    app_ico.write(ico_out).unwrap();

    let mut res = winresource::WindowsResource::new();
    res.set_icon("build/Puppet Inspector.ico");
    res.compile().unwrap();
}

#[cfg(not(all(target_os = "windows", feature = "branding")))]
fn add_windows_icon() {
    unreachable!();
}

fn main() {
    glib_build_tools::compile_resources(
        &["resources", "src"],
        "resources/resources.gresource.xml",
        "resources.gresource",
    );

    if cfg!(all(target_os = "windows", feature = "branding")) {
        add_windows_icon();
    }
}
