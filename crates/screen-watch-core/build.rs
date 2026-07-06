fn main() {
    println!("cargo:rerun-if-env-changed=SCREENWATCH_BUILD_FLAVOR");

    let build_flavor = std::env::var("SCREENWATCH_BUILD_FLAVOR")
        .ok()
        .map(|value| normalize_flavor(&value))
        .unwrap_or_else(|| {
            if std::env::var_os("CARGO_FEATURE_OCR").is_some() {
                "full"
            } else {
                "lite"
            }
        });

    println!("cargo:rustc-env=SCREENWATCH_COMPILED_BUILD_FLAVOR={build_flavor}");
}

fn normalize_flavor(value: &str) -> &'static str {
    if value.trim().eq_ignore_ascii_case("full") {
        "full"
    } else {
        "lite"
    }
}
