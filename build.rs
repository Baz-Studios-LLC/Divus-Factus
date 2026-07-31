//! Build script: on Windows, stamps the cairn icon into the executable so the
//! taskbar, Explorer and the title bar all show Divus Factus's mark instead of the
//! default exe glyph. Everywhere else this is a no-op; macOS gets its icon
//! from the .app bundle (see packaging/macos-app.sh).

fn main() {
    println!("cargo:rerun-if-changed=packaging/DivusFactus.ico");
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("packaging/DivusFactus.ico");
        res.compile().expect("embedding the Windows icon");
    }
}
