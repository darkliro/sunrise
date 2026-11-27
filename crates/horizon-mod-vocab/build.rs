fn main() {
    let root =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-env-changed=SUNRISE_POINT_NAMES");
    if std::env::var_os("SUNRISE_POINT_NAMES").is_none() {
        horizon_mod_api_gen::generate_vocab_and_engine_points(&root, &out_dir)
            .expect("generating the mod API and engine points");
    } else {
        horizon_mod_api_gen::generate(&root, &out_dir, horizon_mod_api_gen::Target::Vocab)
            .expect("generating the mod API");
    }
    println!("cargo::metadata=out_dir={}", out_dir.display());
    // Test input follows this build's generated enum, not a checked-in hook list.
    let generated = std::fs::read_to_string(out_dir.join("mod_api_generated.rs"))
        .expect("reading generated hook identifiers");
    let variants = generated
        .split_once("pub enum HookId {")
        .expect("generated HookId declaration")
        .1
        .split_once('}')
        .expect("generated HookId variants")
        .0;
    let names = variants
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    std::fs::write(
        out_dir.join("hook_id_variant_names.rs"),
        format!("{names:?}"),
    )
    .expect("writing same-source hook layout test input");
}
