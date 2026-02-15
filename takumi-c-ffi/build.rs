use std::{env, path::PathBuf};

fn main() {
  let crate_dir =
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));

  let config_path = crate_dir.join("cbindgen.toml");
  let output_path = crate_dir.join("include").join("takumi.h");

  println!("cargo:rerun-if-changed=src");
  println!("cargo:rerun-if-changed={}", config_path.display());

  let config = cbindgen::Config::from_file(&config_path).expect("failed to load cbindgen.toml");

  cbindgen::Builder::new()
    .with_crate(crate_dir)
    .with_config(config)
    .generate()
    .expect("failed to generate C header")
    .write_to_file(output_path);
}
