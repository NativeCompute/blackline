// windows icon here---
fn main() {
    println!("cargo:rerun-if-changed=packaging/blackline.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/blackline.ico");
        res.set("ProductName", "Blackline");
        res.set("FileDescription", "Blackline");
        res.set("CompanyName", "NativeCompute");
        res.set("LegalCopyright", "Copyright (c) 2026 NativeCompute");
        if let Err(e) = res.compile() {
            panic!("could not embed the Windows icon: {e}");
        }
    }
}
