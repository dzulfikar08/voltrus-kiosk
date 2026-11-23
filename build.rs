//! Windows: put the Voltrus icon and version into the .exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon/icon.ico")
                .set("FileDescription", "Voltrus Kiosk")
                .set("ProductName", "Voltrus Kiosk")
                .set("CompanyName", "PT Joyo Digitama Solusi");
            res.compile().expect("embed the Windows icon");
        }
    }
}
