use std::io::Cursor;
use takumi_raster::{Bitmap, OutputFormat, write_image};
use tiff::decoder::{Decoder, DecodingResult};

fn cmyk_of(format: OutputFormat) -> Vec<u8> {
  let bitmap = Bitmap::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
  let mut out = Vec::new();
  write_image(&bitmap, &mut out, format).unwrap();
  let mut decoder = Decoder::new(Cursor::new(out)).unwrap();
  assert_eq!(decoder.colortype().unwrap(), tiff::ColorType::CMYK(8));
  match decoder.read_image().unwrap() {
    DecodingResult::U8(v) => v,
    _ => panic!(),
  }
}

#[test]
fn naive() {
  assert_eq!(
    cmyk_of(OutputFormat::TiffCmyk { profile: None }),
    vec![0, 255, 255, 0, 0, 0, 0, 0]
  );
}

#[test]
fn icc() {
  let icc: &'static [u8] = Box::leak(
    std::fs::read("/System/Library/ColorSync/Profiles/Generic CMYK Profile.icc")
      .unwrap()
      .into_boxed_slice(),
  );
  let v = cmyk_of(OutputFormat::TiffCmyk { profile: Some(icc) });
  eprintln!("icc cmyk = {v:?}");
  assert!(
    v[0] < 60 && v[1] > 180 && v[2] > 180 && v[3] < 60,
    "red -> {v:?}"
  );
  assert!(
    v[4..].iter().all(|&x| x < 12),
    "transparent -> {:?}",
    &v[4..]
  );
}
