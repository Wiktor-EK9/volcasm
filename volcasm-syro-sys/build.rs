use std::env;
use std::path::PathBuf;
use std::process::Command;

const SYRO_DIR: &str = "vendor/volcasample/syro";
const SYRO_SOURCES: [&str; 3] = [
    "korg_syro_comp.c",
    "korg_syro_func.c",
    "korg_syro_volcasample.c",
];
const EXAMPLE_SOURCE: &str = "vendor/volcasample/example/korg_syro_volcasample_example.c";

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=vendor/volcasample");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // The SDK predates C23 (`typedef int bool` in korg_syro_type.h), so pin the
    // dialect GCC defaulted to at the time (MSVC has no C23 `bool` keyword
    // and no such dialect). Vendored code is left untouched apart from
    // `patches/` (D24), hence warnings are silenced.
    let mut build = cc::Build::new();
    build.include(SYRO_DIR).warnings(false);
    if !build.get_compiler().is_like_msvc() {
        build.std("gnu11");
    }
    for src in SYRO_SOURCES {
        build.file(format!("{SYRO_DIR}/{src}"));
    }
    build.compile("syro_volcasample");

    if env::var_os("CARGO_FEATURE_REFERENCE_EXAMPLE").is_some() {
        build_reference_example(&build, &out_dir);
    }

    // Only the pattern types are bound; volcasample_pattern.c is not compiled.
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{SYRO_DIR}"))
        .allowlist_function("SyroVolcaSample_.*")
        .allowlist_type("Syro.*|Endian|VolcaSample_.*_Data")
        .allowlist_var("VOLCASAMPLE_.*|SYRO_VOLCASAMPLE_.*")
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("failed to generate SYRO bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write SYRO bindings");
}

/// Builds the unmodified SDK example (`syro_volcasample_example`) as the
/// reference for bit-exact stream comparisons in tests.
fn build_reference_example(build: &cc::Build, out_dir: &std::path::Path) {
    let compiler = build.get_compiler();
    let mut cmd: Command = compiler.to_command();
    let exe = if compiler.is_like_msvc() {
        let exe = out_dir.join("syro_volcasample_example.exe");
        cmd.arg("/nologo")
            .arg("/w")
            .arg(format!("/Fe{}", exe.display()));
        cmd.arg(format!("/Fo{}\\", out_dir.display()));
        exe
    } else {
        let exe = out_dir.join("syro_volcasample_example");
        cmd.arg("-std=gnu11").arg("-w").arg("-o").arg(&exe);
        exe
    };
    for src in SYRO_SOURCES {
        cmd.arg(format!("{SYRO_DIR}/{src}"));
    }
    cmd.arg(EXAMPLE_SOURCE);
    let status = cmd.status().expect("failed to run C compiler");
    assert!(status.success(), "failed to build SDK example");
    println!("cargo:rustc-env=VOLCASM_SYRO_EXAMPLE_BIN={}", exe.display());
}
