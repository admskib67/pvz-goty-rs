fn main() {
    #[cfg(windows)]
    {
        let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../github/pvz.ico");
        let mut resource = winres::WindowsResource::new();
        resource.set_icon(icon.to_str().expect("icon path is UTF-8"));
        resource.set("ProductName", "Plants vs. Zombies");
        resource.set("FileDescription", "Plants vs. Zombies");
        resource.set("OriginalFilename", "pvz.exe");
        if let Err(e) = resource.compile() {
            panic!("failed to embed the Windows icon: {e}");
        }
    }
}
