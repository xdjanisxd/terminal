fn main() {
    println!("cargo:rerun-if-changed=../../assets/branding/windows/terminal.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/branding/windows/terminal.ico")
            .set("FileDescription", "Terminal")
            .set("ProductName", "Terminal")
            .set("InternalName", "terminal")
            .set("OriginalFilename", "terminal.exe")
            .compile()
            .expect("could not compile Terminal Windows resources");
    }
}
