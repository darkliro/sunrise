fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=SUNRISE_POINT_NAMES");
    if std::env::var_os("SUNRISE_POINT_NAMES").is_none()
        && let Some(vocab_out) = std::env::var_os("DEP_HORIZON_MOD_VOCAB_OUT_DIR")
    {
        let prebuilt = std::path::PathBuf::from(vocab_out).join("engine_points.rs");
        if prebuilt.exists() {
            println!("cargo:rerun-if-changed={}", prebuilt.display());
            std::fs::copy(&prebuilt, out.join("engine_points.rs"))
                .expect("copying prebuilt engine points");
            return;
        }
    }
    let root =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    horizon_mod_api_gen::generate_engine_points(&root, &out).expect("generating engine points");
}
