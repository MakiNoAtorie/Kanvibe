// Embeds the K logo as the exe's icon, so Explorer and shortcuts show it.
fn main() {
    println!("cargo:rerun-if-changed=assets/kanvibe.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/kanvibe.ico")
            .compile()
            .expect("embed the exe icon");
    }
}
