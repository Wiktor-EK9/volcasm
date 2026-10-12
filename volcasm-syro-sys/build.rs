use std::env;
use std::path::PathBuf;

const SYRO_DIR: &str = "vendor/volcasample/syro";
const SYRO_SOURCES: [&str; 3] = [
    "korg_syro_comp.c",
    "korg_syro_func.c",
    "korg_syro_volcasample.c",
];

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed={SYRO_DIR}");

    // The SDK predates C23 (`typedef int bool` in korg_syro_type.h), so pin the
    // dialect GCC defaulted to at the time. Vendored code is left untouched,
    // hence warnings are silenced.
    let mut build = cc::Build::new();
    build.include(SYRO_DIR).std("gnu11").warnings(false);
    for src in SYRO_SOURCES {
        build.file(format!("{SYRO_DIR}/{src}"));
    }
    build.compile("syro_volcasample");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .allowlist_function("SyroVolcaSample_.*")
        .allowlist_type("Syro.*|Endian")
        .allowlist_var("VOLCASAMPLE_.*|SYRO_VOLCASAMPLE_.*")
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("failed to generate SYRO bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out)
        .expect("failed to write SYRO bindings");
}
