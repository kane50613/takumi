use crate::subsetter::read::{Readable, Reader};
use crate::subsetter::write::{Writeable, Writer};
use std::fmt::{Debug, Formatter};

const FLOAT_STACK_LEN: usize = 64;
const END_OF_FLOAT_FLAG: u8 = 0xf;

#[derive(Clone, Copy)]
pub struct RealNumber(f32);

impl Debug for RealNumber {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Clone, Default, Eq, Copy, PartialEq)]
pub struct IntegerNumber(pub i32);

impl Debug for IntegerNumber {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Clone, Copy)]
pub struct FixedNumber(i32);

impl Debug for FixedNumber {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl FixedNumber {
  pub fn as_f32(&self) -> f32 {
    self.0 as f32 / 65536.0
  }

  pub fn parse(r: &mut Reader<'_>) -> Option<Self> {
    let b0 = r.read::<u8>()?;

    if b0 != 255 {
      return None;
    }

    let num = r.read::<i32>()?;
    Some(FixedNumber(num))
  }
}

impl Writeable for FixedNumber {
  fn write(&self, w: &mut Writer) {
    w.write::<u8>(255);
    w.write(self.0);
  }
}

impl RealNumber {
  // The parsing logic was taken from ttf-parser.
  pub fn parse(r: &mut Reader) -> Option<RealNumber> {
    let mut data = [0u8; FLOAT_STACK_LEN];
    let mut idx = 0;

    let b0 = r.read::<u8>()?;

    if b0 != 30 {
      return None;
    }

    loop {
      let b1: u8 = r.read()?;
      let nibble1 = b1 >> 4;
      let nibble2 = b1 & 15;

      if nibble1 == END_OF_FLOAT_FLAG {
        break;
      }

      idx = parse_float_nibble(nibble1, idx, &mut data)?;

      if nibble2 == END_OF_FLOAT_FLAG {
        break;
      }

      idx = parse_float_nibble(nibble2, idx, &mut data)?;
    }

    let s = core::str::from_utf8(&data[..idx]).ok()?;
    let n = s.parse().ok()?;

    Some(RealNumber(n))
  }
}

impl Writeable for RealNumber {
  // Not the fastest implementation, but floats don't appear that often anyway,
  // so it's good enough.
  fn write(&self, w: &mut Writer) {
    let mut nibbles = vec![];

    let string_form = format!("{}", self.0);
    let mut r = Reader::new(string_form.as_bytes());

    while !r.at_end() {
      let byte = r.read::<u8>().unwrap();

      match byte {
        b'0'..=b'9' => nibbles.push(byte - 48),
        b'.' => nibbles.push(0xA),
        b'-' => nibbles.push(0xE),
        _ => unreachable!(),
      }
    }

    nibbles.push(0xF);

    if nibbles.len() % 2 != 0 {
      nibbles.push(0xF);
    }

    // Prefix of fixed number.
    w.write::<u8>(30);

    for (first, second) in nibbles.chunks(2).map(|pair| (pair[0], pair[1])) {
      let num = (first << 4) | second;
      w.write(num);
    }
  }
}

impl IntegerNumber {
  pub fn parse(r: &mut Reader<'_>) -> Option<IntegerNumber> {
    let b0 = r.read::<u8>()?;
    match b0 {
      28 => Some(IntegerNumber(i32::from(r.read::<i16>()?))),
      29 => Some(IntegerNumber(r.read::<i32>()?)),
      32..=246 => {
        let n = i32::from(b0) - 139;
        Some(IntegerNumber(n))
      }
      247..=250 => {
        let b1 = i32::from(r.read::<u8>()?);
        let n = (i32::from(b0) - 247) * 256 + b1 + 108;
        Some(IntegerNumber(n))
      }
      251..=254 => {
        let b1 = i32::from(r.read::<u8>()?);
        let n = -(i32::from(b0) - 251) * 256 - b1 - 108;
        Some(IntegerNumber(n))
      }
      _ => None,
    }
  }

  /// Write the number as a 5 byte sequence. This is necessary when writing offsets,
  /// because we need to the length of the number to stay stable, since it would
  /// otherwise shift everything.
  pub fn write_as_5_bytes(&self, w: &mut Writer) {
    let bytes = self.0.to_be_bytes();
    w.write([29, bytes[0], bytes[1], bytes[2], bytes[3]]);
  }
}

impl Writeable for IntegerNumber {
  fn write(&self, w: &mut Writer) {
    if (-107..=107).contains(&self.0) {
      let b0 = u8::try_from(self.0 + 139).unwrap();
      w.write(b0);
    } else if (108..=1131).contains(&self.0) {
      let temp = self.0 - 108;
      let b0 = u8::try_from(temp / 256 + 247).unwrap();
      let b1 = u8::try_from(temp % 256).unwrap();
      w.write([b0, b1]);
    } else if (-1131..=-108).contains(&self.0) {
      let temp = -self.0 - 108;
      let b0 = u8::try_from(temp / 256 + 251).unwrap();
      let b1 = u8::try_from(temp % 256).unwrap();
      w.write([b0, b1])
    } else if (-32768..=32767).contains(&self.0) {
      let bytes = i16::try_from(self.0).unwrap().to_be_bytes();
      w.write([28, bytes[0], bytes[1]])
    } else {
      self.write_as_5_bytes(w)
    }
  }
}

#[derive(Clone, Copy)]
pub enum Number {
  Real(RealNumber),
  Integer(IntegerNumber),
  Fixed(FixedNumber),
}

impl Default for Number {
  fn default() -> Self {
    Number::zero()
  }
}

impl Debug for Number {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_f64())
  }
}

impl Writeable for Number {
  fn write(&self, w: &mut Writer) {
    match self {
      Number::Real(real_num) => real_num.write(w),
      Number::Integer(int_num) => int_num.write(w),
      Number::Fixed(fixed_num) => fixed_num.write(w),
    }
  }
}

impl Number {
  pub fn parse_cff_number(r: &mut Reader) -> Option<Number> {
    Self::parse_number(r, false)
  }

  pub fn parse_char_string_number(r: &mut Reader) -> Option<Number> {
    Self::parse_number(r, true)
  }

  fn parse_number(r: &mut Reader, charstring_num: bool) -> Option<Number> {
    match r.peak::<u8>()? {
      30 => Some(Number::Real(RealNumber::parse(r)?)),
      // FIXED only exists in charstrings.
      255 => {
        if charstring_num {
          return Some(Number::Fixed(FixedNumber::parse(r)?));
        }

        None
      }
      _ => Some(Number::Integer(IntegerNumber::parse(r)?)),
    }
  }

  pub fn from_i32(num: i32) -> Self {
    Number::Integer(IntegerNumber(num))
  }

  pub fn from_f32(num: f32) -> Self {
    Number::Real(RealNumber(num))
  }

  pub fn zero() -> Self {
    Number::Integer(IntegerNumber(0))
  }

  pub fn one() -> Self {
    Number::Integer(IntegerNumber(1))
  }

  pub fn as_f64(&self) -> f64 {
    match self {
      Number::Integer(int) => int.0 as f64,
      Number::Real(real) => real.0 as f64,
      Number::Fixed(fixed) => fixed.as_f32() as f64,
    }
  }

  pub fn as_i32(&self) -> Option<i32> {
    match self {
      Number::Integer(int) => Some(int.0),
      Number::Real(rn) => {
        if rn.0.fract() == 0.0 {
          Some(rn.0 as i32)
        } else {
          None
        }
      }
      Number::Fixed(fixn) => {
        let num = fixn.as_f32();
        if num.fract() == 0.0 {
          Some(num as i32)
        } else {
          None
        }
      }
    }
  }

  pub fn as_u32(&self) -> Option<u32> {
    u32::try_from(self.as_i32()?).ok()
  }

  // Adapted from ttf_parser's Transform::combine
  pub fn combine(t1: [Self; 6], t2: [Self; 6]) -> [Self; 6] {
    [
      Number::from_f32((t1[0].as_f64() * t2[0].as_f64() + t1[2].as_f64() * t2[1].as_f64()) as f32),
      Number::from_f32((t1[1].as_f64() * t2[0].as_f64() + t1[3].as_f64() * t2[1].as_f64()) as f32),
      Number::from_f32((t1[0].as_f64() * t2[2].as_f64() + t1[2].as_f64() * t2[3].as_f64()) as f32),
      Number::from_f32((t1[1].as_f64() * t2[2].as_f64() + t1[3].as_f64() * t2[3].as_f64()) as f32),
      Number::from_f32(
        (t1[0].as_f64() * t2[4].as_f64() + t1[2].as_f64() * t2[5].as_f64() + t1[4].as_f64()) as f32,
      ),
      Number::from_f32(
        (t1[1].as_f64() * t2[4].as_f64() + t1[3].as_f64() * t2[5].as_f64() + t1[5].as_f64()) as f32,
      ),
    ]
  }
}

fn parse_float_nibble(nibble: u8, mut idx: usize, data: &mut [u8]) -> Option<usize> {
  if idx == FLOAT_STACK_LEN {
    return None;
  }

  match nibble {
    0..=9 => {
      data[idx] = b'0' + nibble;
    }
    10 => {
      data[idx] = b'.';
    }
    11 => {
      data[idx] = b'E';
    }
    12 => {
      if idx + 1 == FLOAT_STACK_LEN {
        return None;
      }

      data[idx] = b'E';
      idx += 1;
      data[idx] = b'-';
    }
    13 => {
      return None;
    }
    14 => {
      data[idx] = b'-';
    }
    _ => {
      return None;
    }
  }

  idx += 1;
  Some(idx)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug, Hash, Ord)]
pub struct StringId(pub u16);

impl StringId {
  pub const STANDARD_STRING_LEN: u16 = 391;

  pub fn is_standard_string(&self) -> bool {
    self.0 < Self::STANDARD_STRING_LEN
  }
}

impl Readable<'_> for StringId {
  const SIZE: usize = u16::SIZE;

  fn read(r: &mut Reader<'_>) -> Option<Self> {
    Some(Self(r.read::<u16>()?))
  }
}

impl Writeable for StringId {
  fn write(&self, w: &mut Writer) {
    w.write::<u16>(self.0)
  }
}

impl From<u16> for StringId {
  fn from(value: u16) -> Self {
    Self(value)
  }
}

#[derive(Clone, Copy, Debug)]
pub struct U24(pub u32);

impl U24 {
  pub const MAX: u32 = 16777215;
}

impl Readable<'_> for U24 {
  const SIZE: usize = 3;

  fn read(r: &mut Reader<'_>) -> Option<Self> {
    let data = r.read::<[u8; 3]>()?;
    Some(U24(u32::from_be_bytes([0, data[0], data[1], data[2]])))
  }
}

impl Writeable for U24 {
  fn write(&self, w: &mut Writer) {
    let data = self.0.to_be_bytes();
    w.write::<[u8; 3]>([data[1], data[2], data[3]]);
  }
}

#[cfg(test)]
mod tests {
  use crate::subsetter::cff::number::*;
  use crate::subsetter::read::Reader;

  #[test]
  fn u24() {
    let nums = [0u32, 45, 345, 54045, 32849324, 16777215];

    for num in nums {
      let wrapped = U24(num);

      let mut w = Writer::new();
      w.write(wrapped);
      let first = w.finish();

      let mut r = Reader::new(&first);
      let rewritten = r.read::<U24>().unwrap();
      let mut w = Writer::new();
      w.write(rewritten);
      let second = w.finish();

      assert_eq!(first, second);
    }
  }

  #[test]
  fn size1_roundtrip() {
    let nums = [0, 1, -1, 93, 107, -107];

    for num in nums {
      let integer = IntegerNumber(num);
      let mut w = Writer::new();
      w.write(integer);
      let buffer = w.finish();
      let mut reader = Reader::new(&buffer);

      let reparsed = IntegerNumber::parse(&mut reader).unwrap();
      let mut w = Writer::new();
      w.write(reparsed);
      let bytes = w.finish();
      assert_eq!(bytes.len(), 1);
      assert_eq!(reparsed.0, num);
    }
  }

  #[test]
  fn size2_roundtrip() {
    let nums = [108, -108, 255, -255, 349, -349, 845, -845, 1131, -1131];

    for num in nums {
      let integer = IntegerNumber(num);
      let mut w = Writer::new();
      w.write(integer);
      let buffer = w.finish();
      let mut reader = Reader::new(&buffer);

      let reparsed = IntegerNumber::parse(&mut reader).unwrap();
      let mut w = Writer::new();
      w.write(reparsed);
      let bytes = w.finish();
      assert_eq!(bytes.len(), 2);
      assert_eq!(reparsed.0, num);
    }
  }

  #[test]
  fn size3_roundtrip() {
    let nums = [
      1132, -1132, 2450, -2450, 4096, -4096, 8965, -8965, 32767, -32768,
    ];

    for num in nums {
      let integer = IntegerNumber(num);
      let mut w = Writer::new();
      w.write(integer);
      let buffer = w.finish();
      let mut reader = Reader::new(&buffer);

      let reparsed = IntegerNumber::parse(&mut reader).unwrap();
      let mut w = Writer::new();
      w.write(reparsed);
      let bytes = w.finish();
      assert_eq!(bytes.len(), 3);
      assert_eq!(reparsed.0, num);
    }
  }

  #[test]
  fn size5_roundtrip() {
    let nums = [32768, -32769, i32::MAX, i32::MIN];

    for num in nums {
      let integer = IntegerNumber(num);
      let mut w = Writer::new();
      w.write(integer);
      let buffer = w.finish();
      let mut reader = Reader::new(&buffer);

      let reparsed = IntegerNumber::parse(&mut reader).unwrap();
      let mut w = Writer::new();
      w.write(reparsed);
      let bytes = w.finish();
      assert_eq!(bytes.len(), 5);
      assert_eq!(reparsed.0, num);
    }
  }

  #[test]
  fn parse_float() {
    let num = [0x1E, 0xE2, 0x49, 0x32, 0xA1, 0x2C, 0x2F];
    let mut r = Reader::new(&num);
    let real = RealNumber::parse(&mut r).unwrap();
    assert_eq!(-249.3212, real.0);
  }

  #[test]
  fn float_roundtrip() {
    let nums = [0.58f32, -0.21, 3.98, 16.49, 159.18, 5906.2];

    for num in nums {
      let float = RealNumber(num);
      let mut w = Writer::new();
      w.write(float);
      let buffer = w.finish();
      let mut reader = Reader::new(&buffer);

      let reparsed = RealNumber::parse(&mut reader).unwrap();
      assert_eq!(reparsed.0, num);
    }
  }

  #[test]
  fn fixed() {
    let num = [255u8, 154, 104, 120, 40];

    let mut r = Reader::new(&num);
    let parsed = FixedNumber::parse(&mut r).unwrap();

    let mut w = Writer::new();
    w.write(parsed);

    assert_eq!(num, w.finish().as_ref());
  }
}
