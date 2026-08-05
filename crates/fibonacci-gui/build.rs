fn main() {
    // The Logalith rides the exe: icon/icon.ico into the Windows resource
    // table (Explorer, taskbar, alt-tab). Skipped entirely off-Windows, so
    // building from source elsewhere needs no resource compiler.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("icon/icon.ico")
            .compile()
            .expect("embedding the icon resource");
    }
    println!("cargo:rerun-if-changed=icon/icon.ico");
}
