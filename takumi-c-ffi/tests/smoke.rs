use std::{
  env,
  path::{Path, PathBuf},
  process::Command,
};

fn target_debug_dir(manifest_dir: &Path) -> PathBuf {
  let mut dir = manifest_dir.to_path_buf();
  dir.pop();
  dir.push("target");
  dir.push("debug");
  dir
}

#[test]
fn c_api_all_functions_smoke() {
  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let c_test_src = manifest_dir.join("tests").join("all_functions.c");
  let include_dir = manifest_dir.join("include");
  let debug_dir = target_debug_dir(&manifest_dir);

  let dylib_name = if cfg!(target_os = "macos") {
    "libtakumi_c_ffi.dylib"
  } else if cfg!(target_os = "windows") {
    "takumi_c_ffi.dll"
  } else {
    "libtakumi_c_ffi.so"
  };
  let dylib_path = debug_dir.join(dylib_name);
  assert!(dylib_path.exists(), "missing c ffi dylib at {dylib_path:?}");

  let font_path = manifest_dir
    .parent()
    .expect("workspace root")
    .join("assets")
    .join("fonts")
    .join("archivo")
    .join("Archivo-VariableFont_wdth,wght.ttf");
  assert!(font_path.exists(), "missing test font at {font_path:?}");
  let image_path = manifest_dir
    .parent()
    .expect("workspace root")
    .join("assets")
    .join("images")
    .join("yeecord.png");
  assert!(image_path.exists(), "missing test image at {image_path:?}");

  let out_bin = debug_dir.join(if cfg!(target_os = "windows") {
    "c_api_all_functions.exe"
  } else {
    "c_api_all_functions"
  });

  let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());

  let mut compile = Command::new(&cc);
  compile
    .arg("-std=c11")
    .arg("-Wall")
    .arg("-Wextra")
    .arg("-pedantic")
    .arg(&c_test_src)
    .arg(format!("-I{}", include_dir.display()))
    .arg(format!("-L{}", debug_dir.display()))
    .arg("-ltakumi_c_ffi")
    .arg("-o")
    .arg(&out_bin);

  let compile_output = compile
    .output()
    .expect("failed to invoke C compiler for c_api_all_functions test");
  assert!(
    compile_output.status.success(),
    "C compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile_output.stdout),
    String::from_utf8_lossy(&compile_output.stderr)
  );

  let mut run = Command::new(&out_bin);
  run.arg(&font_path).arg(&image_path);
  if cfg!(target_os = "macos") {
    run.env("DYLD_LIBRARY_PATH", &debug_dir);
  } else if cfg!(target_os = "windows") {
    run.env(
      "PATH",
      format!(
        "{};{}",
        debug_dir.display(),
        env::var("PATH").unwrap_or_default()
      ),
    );
  } else {
    run.env("LD_LIBRARY_PATH", &debug_dir);
  }

  let run_output = run.output().expect("failed to run C smoke test binary");
  assert!(
    run_output.status.success(),
    "C smoke test failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&run_output.stdout),
    String::from_utf8_lossy(&run_output.stderr)
  );
}
