fn main() {
    println!("cargo:rerun-if-changed=errors.cpp");
    println!("cargo:rerun-if-changed=primary.cpp");
    println!("cargo:rerun-if-changed=../../unrar_sys/vendor/unrar");
    let mut build = cc::Build::new();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        build.flag("/EHsc");
    }
    build
        .cpp(true)
        .std("c++14")
        .warnings(false)
        .flag_if_supported("-stdlib=libc++")
        .define("RARDLL", None)
        .define("RAR_SMP", None)
        .define("_FILE_OFFSET_BITS", "64")
        .define("_LARGEFILE_SOURCE", None)
        .include("../../unrar_sys/vendor/unrar")
        .file("errors.cpp")
        .file("primary.cpp")
        .compile("unrar_native_tests");
}
