fn main() {
    println!("cargo:rerun-if-changed=icons/harmony.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/harmony.ico");
        res.set("ProductName", "Harmony");
        res.set("FileDescription", "Harmony time tracker");
        // Needs rc.exe from the Windows SDK; the app still works without the icon.
        if let Err(e) = res.compile() {
            println!("cargo:warning=exe icon skipped: {e}");
        }
    }
}
