fn main() {
    // Our own linker script instead of esp-hal's `linkall.x`: the ROM only
    // loads RAM segments of a second-stage bootloader, so *everything* --
    // code, rodata, data -- must live in RAM (see boot.x/boot-esp32s3.x).
    let linker = if std::env::var_os("CARGO_FEATURE_ESP32C3").is_some() {
        "boot.x"
    } else if std::env::var_os("CARGO_FEATURE_ESP32S3").is_some() {
        "boot-esp32s3.x"
    } else {
        panic!("no supported ESP boot target feature selected");
    };
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search={dir}");
    println!("cargo:rustc-link-arg=-T{linker}");
    println!("cargo:rerun-if-changed={linker}");
}
