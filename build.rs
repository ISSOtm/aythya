use std::{path::PathBuf, process::Command};

fn main() {
    let version_file =
        std::fs::read_to_string("SameBoy/version.mk").expect("Unable to read SameBoy version file");
    let version = version_file
        .split_once("=")
        .expect("No `=` character in SameBoy version file")
        .1
        .trim();
    let mut sameboy = cc::Build::new();
    sameboy
        .define("_GNU_SOURCE", None)
        .define("_USE_MATH_DEFINES", None)
        .include("SameBoy/")
        .define("GB_INTERNAL", None)
        .define("GB_VERSION", Some(format!("\"{version}\"").as_str()))
        .extra_warnings(false)
        .flag("-Wno-multichar")
        .flag("-Wno-missing-braces");
    for entry in std::fs::read_dir("SameBoy/Core/").expect("Unable to enumerate `SameBoy/Core/`") {
        let file = entry.expect("Error enumerating `SameBoy/Core/`");
        if file.file_name().to_str().unwrap().ends_with(".c") {
            sameboy.file(file.path());
        }
    }
    sameboy.compile("sameboy");

    let bindings = bindgen::builder()
        .header("SameBoy/Core/gb.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate SameBoy bindings");
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write SameBoy bindings");

    slint_build::compile("ui/main.slint").expect("Unable to compile Slint files");

    assert!(
        Command::new("make")
            .args(["-CSameBoy", "bootroms"])
            .status()
            .expect("Unable to compile boot ROMs")
            .success()
    );
}
