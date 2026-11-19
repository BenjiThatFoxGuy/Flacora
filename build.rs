//! Embeds Flacora's icon and third-party notices in the Windows executable.
//! The window and taskbar icon is set at runtime with `rune::set_app_icon`.

fn main() {
    println!("cargo:rerun-if-changed=assets/appicon.ico");
    println!("cargo:rerun-if-changed=assets/astryx.LICENSE");
    println!("cargo:rerun-if-changed=assets/figtree.OFL");
    println!("cargo:rerun-if-changed=assets/lucide.LICENSE");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/appicon.ico");
        resource.append_rc_content(
            r#"
ASTRYX_LICENSE RCDATA "assets/astryx.LICENSE"
FIGTREE_LICENSE RCDATA "assets/figtree.OFL"
LUCIDE_LICENSE RCDATA "assets/lucide.LICENSE"
"#,
        );
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Windows executable resources not embedded: {error}");
        }
    }
}
