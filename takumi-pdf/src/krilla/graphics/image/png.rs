//! Reads a PNG's chunks to decide whether its IDAT stream can be embedded as
//! is. See the PNG spec: https://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html

use png::chunk;

pub(super) struct PngData {
  pub(super) idat: Vec<u8>,
  pub(super) bit_depth: png::BitDepth,
  pub(super) color_type: png::ColorType,
  pub(super) palette: Option<Vec<u8>>,
}

impl PngData {
  pub(super) fn new(data: &[u8]) -> Option<Self> {
    let mut chunks = Chunks::new(data).ok()?;
    let ihdr = chunks.next()?.ok()?;
    if ihdr.kind != chunk::IHDR || !ihdr.has_valid_crc() {
      return None;
    }

    let header = Header::new(ihdr.data)?;

    if header.interlaced
      || !matches!(
        header.color_type,
        png::ColorType::Grayscale | png::ColorType::Rgb | png::ColorType::Indexed
      )
    {
      return None;
    }

    let mut idat = None;
    let mut palette = None;
    let mut palette_seen = false;
    let mut reached_iend = false;

    for png_chunk in chunks {
      let png_chunk = png_chunk.ok()?;

      if png_chunk.is_critical && !png_chunk.has_valid_crc() {
        return None;
      }

      match png_chunk.kind {
        chunk::IHDR | chunk::tRNS => return None,
        chunk::PLTE => {
          let max_len = match header.color_type {
            png::ColorType::Indexed => 3 * (1 << header.bit_depth as usize),
            png::ColorType::Rgb => 3 * 256,
            _ => return None,
          };

          if palette_seen
            || idat.is_some()
            || png_chunk.data.is_empty()
            || png_chunk.data.len() % 3 != 0
            || png_chunk.data.len() > max_len
          {
            return None;
          }

          palette_seen = true;

          // See 4.1.2: https://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html
          // For RGB images, we can safely ignore it.
          if header.color_type == png::ColorType::Indexed {
            palette = Some(png_chunk.data.to_vec());
          }
        }
        chunk::IDAT => {
          idat
            .get_or_insert_with(Vec::new)
            .extend_from_slice(png_chunk.data);
        }
        chunk::IEND => {
          reached_iend = true;
          break;
        }
        _ if png_chunk.is_critical => return None,
        _ => {}
      }
    }

    if header.color_type == png::ColorType::Indexed && palette.is_none() {
      return None;
    }

    reached_iend.then_some(Self {
      // Note: For performance reasons we don't inflate and validate whether
      // the actual length of the data is correct. This does mean that
      // invalid data might be embedded in case the original PNG was
      // corrupted.
      idat: idat?,
      bit_depth: header.bit_depth,
      color_type: header.color_type,
      palette,
    })
  }
}

struct Header {
  bit_depth: png::BitDepth,
  color_type: png::ColorType,
  interlaced: bool,
}

impl Header {
  fn new(data: &[u8]) -> Option<Self> {
    let data: &[u8; 13] = data.try_into().ok()?;
    let width = u32::from_be_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
    let bit_depth = png::BitDepth::from_u8(data[8])?;
    let color_type = png::ColorType::from_u8(data[9])?;

    if width == 0 || height == 0 || data[10] != 0 || data[11] != 0 {
      return None;
    }

    let interlaced = match data[12] {
      0 => false,
      1 => true,
      _ => return None,
    };

    Some(Self {
      bit_depth,
      color_type,
      interlaced,
    })
  }
}

const PNG_MAGIC: &[u8] = b"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A";

struct PngChunk<'a> {
  kind: chunk::ChunkType,
  data: &'a [u8],
  crc: u32,
  is_critical: bool,
}

impl PngChunk<'_> {
  fn has_valid_crc(&self) -> bool {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&self.kind.0);
    hasher.update(self.data);
    hasher.finalize() == self.crc
  }
}

struct Chunks<'a> {
  reader: Reader<'a>,
  failed: bool,
}

impl<'a> Chunks<'a> {
  fn new(data: &'a [u8]) -> Result<Self, &'static str> {
    let mut reader = Reader { data };

    if reader.read(PNG_MAGIC.len()) != Some(PNG_MAGIC) {
      return Err("invalid PNG signature");
    }

    Ok(Self {
      reader,
      failed: false,
    })
  }

  fn read_chunk(&mut self) -> Result<PngChunk<'a>, &'static str> {
    let chunk_len = self.reader.read_u32().ok_or("chunk is too short")?;
    let kind = self.reader.read_u32().ok_or("chunk is too short")?;
    let data = self
      .reader
      .read(chunk_len as usize)
      .ok_or("chunk is too short")?;
    let crc = self.reader.read_u32().ok_or("chunk is too short")?;

    let kind = chunk::ChunkType(kind.to_be_bytes());

    Ok(PngChunk {
      kind,
      data,
      crc,
      is_critical: chunk::is_critical(kind),
    })
  }
}

impl<'a> Iterator for Chunks<'a> {
  type Item = Result<PngChunk<'a>, &'static str>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.failed || self.reader.data.is_empty() {
      return None;
    }

    let chunk = self.read_chunk();
    self.failed = chunk.is_err();
    Some(chunk)
  }
}

struct Reader<'a> {
  data: &'a [u8],
}

impl<'a> Reader<'a> {
  fn read(&mut self, len: usize) -> Option<&'a [u8]> {
    let (bytes, rest) = self.data.split_at_checked(len)?;
    self.data = rest;
    Some(bytes)
  }

  fn read_u32(&mut self) -> Option<u32> {
    let bytes = self.read(size_of::<u32>())?;
    Some(u32::from_be_bytes(bytes.try_into().unwrap()))
  }
}
