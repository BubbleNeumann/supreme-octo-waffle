//! Puts the application icon inside the Windows executable.
//!
//! Windows takes the icon for the taskbar, Alt-Tab and Explorer from a resource
//! in the executable, not from the window. Iced can only set the window's own
//! icon, and winit gives that to `ICON_SMALL`, which is the title bar's alone;
//! the window class it registers carries no icon at all. Without the resource
//! below there is nothing for the taskbar to read, and it falls back.
//!
//! Every other platform builds this as an empty main.

fn main() {
    println!("cargo:rerun-if-changed=../../icons/icon.ico");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../icons/icon.ico");

        // Embedding needs a resource compiler from the Windows SDK. Lantern
        // runs perfectly well wearing the system's default icon, so a machine
        // without one gets a warning rather than a failed build.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=the application icon was not embedded: {error}");
        }
    }
}
