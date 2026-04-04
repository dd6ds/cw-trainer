fn main() {
    // Rust's prebuilt std for i686-pc-windows-gnu references GetHostNameW from
    // ws2_32 but doesn't declare the link dependency — add it explicitly here.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=ws2_32");
    }
}
