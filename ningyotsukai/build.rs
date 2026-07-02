use glib_build_tools;

fn main() {
    glib_build_tools::compile_resources(
        &["src"],
        "src/resources.gresource.xml",
        "resources.gresource",
    );

    if cfg!(all(target_os = "windows", feature = "branding")) {
        let mut app_ico = ico::IconDir::new(ico::ResourceType::Icon);

        for path in &[
            "branding/Ningyotsukai Icon 16.png",
            "branding/Ningyotsukai Icon 32.png",
            "branding/Ningyotsukai Icon 64.png",
            "branding/Ningyotsukai Icon 128.png",
        ] {
            println!("cargo:rerun-if-changed={}", path);
            let file = std::fs::File::open(path).unwrap();
            let image = ico::IconImage::read_png(file).unwrap();
            app_ico.add_entry(ico::IconDirEntry::encode(&image).unwrap());
        }

        std::fs::create_dir_all("build").unwrap();

        let ico_out = std::fs::File::create("build/Ningyotsukai.ico").unwrap();
        app_ico.write(ico_out).unwrap();

        let mut res = winresource::WindowsResource::new();
        res.set_icon("build/Ningyotsukai.ico");
        res.compile().unwrap();
    }
}
