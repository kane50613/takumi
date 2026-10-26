#[test]
fn embedded_geist_matches_its_woff2_source() -> Result<(), wuff::WuffErr> {
  let source = include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.woff2");
  let embedded = include_bytes!("../../assets/fonts/geist/geist-latin-wght-300-800.ttf");
  let decoded = wuff::decompress_woff2(source)?;

  assert_eq!(embedded.as_slice(), decoded);

  Ok(())
}
