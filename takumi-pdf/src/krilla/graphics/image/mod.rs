//! Creating and using bitmap images.
//!
//! krilla allows you to add bitmap images to your PDF very easily.
//! The currently supported formats include
//! - PNG
//! - JPG
//! - GIF
//! - WEBP
//! - Custom image formats via [`CustomImage`]

mod png;

use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::ops::DerefMut;
use std::sync::Arc;

use ::png::{BitDepth, ColorType, Transformations};
use pdf_writer::{Dict, Finish, Name, Ref, Str};
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::colorspace::ColorSpace;

use crate::krilla::Data;
use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::error::KrillaError;
use crate::krilla::graphics::color::DEVICE_GRAY;
use crate::krilla::graphics::color::{cmyk, luma, rgb};
use crate::krilla::graphics::icc::{GenericICCProfile, ICCBasedColorSpace, ICCProfile};
use crate::krilla::serialize::{MaybeDeviceColorSpace, SerializeContext};
use crate::krilla::stream::{FilterStreamBuilder, deflate_encode};
use crate::krilla::util::{Deferred, NameExt, SipHashable};

/// The number of bits per color component.
#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum BitsPerComponent {
  /// Eight bits per component.
  Eight,
  /// Sixteen bits per component.
  Sixteen,
}

impl BitsPerComponent {
  fn as_u8(&self) -> u8 {
    match self {
      BitsPerComponent::Eight => 8,
      BitsPerComponent::Sixteen => 16,
    }
  }
}

/// The color space of the image.
#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum ImageColorspace {
  /// The RGB color space.
  Rgb,
  /// The luma color space.
  Luma,
  /// The CMYK color space.
  Cmyk,
}

impl ImageColorspace {
  fn num_components(&self) -> u8 {
    match self {
      ImageColorspace::Luma => 1,
      ImageColorspace::Rgb => 3,
      ImageColorspace::Cmyk => 4,
    }
  }

  fn matches_icc_profile(&self, profile: &GenericICCProfile) -> bool {
    match self {
      ImageColorspace::Rgb => matches!(profile, GenericICCProfile::Rgb(_)),
      ImageColorspace::Luma => matches!(profile, GenericICCProfile::Luma(_)),
      ImageColorspace::Cmyk => matches!(profile, GenericICCProfile::Cmyk(_)),
    }
  }
}

impl TryFrom<ColorSpace> for ImageColorspace {
  type Error = ();

  fn try_from(value: ColorSpace) -> Result<Self, Self::Error> {
    match value {
      ColorSpace::RGB => Ok(ImageColorspace::Rgb),
      ColorSpace::RGBA => Ok(ImageColorspace::Rgb),
      ColorSpace::YCbCr => Ok(ImageColorspace::Rgb),
      ColorSpace::Luma => Ok(ImageColorspace::Luma),
      ColorSpace::LumaA => Ok(ImageColorspace::Luma),
      ColorSpace::YCCK => Ok(ImageColorspace::Cmyk),
      ColorSpace::CMYK => Ok(ImageColorspace::Cmyk),
      _ => Err(()),
    }
  }
}

struct SampledRepr {
  color_channel: Vec<u8>,
  alpha_channel: Option<Vec<u8>>,
  bits_per_component: BitsPerComponent,
}

struct JpegRepr {
  data: Data,
  bits_per_component: BitsPerComponent,
  invert_cmyk: bool,
}

/// A PNG whose IDAT stream is embedded verbatim, without a decode round trip.
struct PngRepr {
  data: Vec<u8>,
  bit_depth: BitDepth,
  color_type: ColorType,
  palette: Option<Vec<u8>>,
}

enum Repr {
  Sampled(SampledRepr),
  Jpeg(JpegRepr),
  Png(PngRepr),
}

impl Repr {
  fn bits_per_component(&self) -> u8 {
    match self {
      Repr::Sampled(s) => s.bits_per_component.as_u8(),
      Repr::Jpeg(j) => j.bits_per_component.as_u8(),
      Repr::Png(p) => p.bit_depth as u8,
    }
  }
}

/// A trait for custom images, which you can use if the
/// current methods provided by krilla (JPEG/PNG/WEBP/GIF) images
/// are not suitable for your own purpose.
///
/// Note that a struct implementing this trait should be cheap to
/// hash and clone, otherwise performance might be bad!
pub trait CustomImage: Hash + Clone + Send + Sync + 'static {
  /// Return the raw bytes of the color channel.
  fn color_channel(&self) -> &[u8];
  /// Return the raw bytes of the alpha channel, if available.
  fn alpha_channel(&self) -> Option<&[u8]>;
  /// Return the bits per component of the image.
  fn bits_per_component(&self) -> BitsPerComponent;
  /// Return the dimensions of the image.
  fn size(&self) -> (u32, u32);
  /// Return the ICC profile of the image, if available.
  fn icc_profile(&self) -> Option<&[u8]>;
  /// Return the color space of the image.
  fn color_space(&self) -> ImageColorspace;
}

struct ImageMetadata {
  size: (u32, u32),
  color_space: ImageColorspace,
  has_alpha: bool,
  bits_per_component: BitsPerComponent,
  icc: Option<GenericICCProfile>,
}

struct ImageRepr {
  inner: Deferred<Result<Repr, String>>,
  metadata: ImageMetadata,
  sip: u128,
  interpolate: bool,
}

impl ImageRepr {
  fn size(&self) -> (u32, u32) {
    self.metadata.size
  }

  fn icc(&self) -> Option<GenericICCProfile> {
    self.metadata.icc.clone()
  }

  fn color_space(&self) -> ImageColorspace {
    self.metadata.color_space
  }
}

impl Debug for ImageRepr {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ImageRepr {{..}}")
  }
}

impl Hash for ImageRepr {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.sip.hash(state);
  }
}

impl PartialEq for ImageRepr {
  fn eq(&self, other: &Self) -> bool {
    self.sip == other.sip
  }
}

impl Eq for ImageRepr {}

/// A bitmap image.
///
/// This type is cheap to hash and clone, but expensive to create.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Image(Arc<ImageRepr>);

impl Image {
  /// Create a new bitmap image from a `.png` file.
  pub fn from_png(data: Data, interpolate: bool) -> Result<Image, String> {
    let hash = data.as_ref().sip_hash();
    let metadata = png_metadata(data.as_ref())?;

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || decode_png(data.as_ref())),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new bitmap image from a `.jpg` file.
  pub fn from_jpeg(data: Data, interpolate: bool) -> Result<Image, String> {
    let hash = data.as_ref().sip_hash();
    let metadata = jpeg_metadata(data.as_ref())?;

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || decode_jpeg(data)),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new bitmap image from a `.jpg` file with custom ICC profile.
  #[doc(hidden)]
  pub fn from_jpeg_with_icc(
    data: Data,
    icc_profile: Option<Data>,
    interpolate: bool,
  ) -> Result<Image, String> {
    let hash = data.as_ref().sip_hash();
    let mut metadata = jpeg_metadata(data.as_ref())?;
    let icc_profile =
      icc_profile.and_then(|d| get_icc_profile_type(d.as_ref(), metadata.color_space));
    metadata.icc = icc_profile;

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || decode_jpeg(data)),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new bitmap image from a `.gif` file.
  pub fn from_gif(data: Data, interpolate: bool) -> Result<Image, String> {
    let hash = data.as_ref().sip_hash();
    let metadata = gif_metadata(data.as_ref())?;

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || decode_gif(data)),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new bitmap image from a `.webp` file.
  ///
  /// Returns `None` if krilla was unable to parse the file.
  pub fn from_webp(data: Data, interpolate: bool) -> Result<Image, String> {
    let hash = data.as_ref().sip_hash();
    let metadata = webp_metadata(data.as_ref())?;

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || decode_webp(data)),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new image from a custom image.
  ///
  /// Panics if the dimensions of the image and the length of the
  /// data doesn't match.
  pub fn from_custom<T: CustomImage>(image: T, interpolate: bool) -> Result<Image, String> {
    let hash = (image.clone(), interpolate).sip_hash();
    let metadata = ImageMetadata {
      size: image.size(),
      color_space: image.color_space(),
      has_alpha: image.alpha_channel().is_some(),
      bits_per_component: image.bits_per_component(),
      icc: image
        .icc_profile()
        .and_then(|d| get_icc_profile_type(d, image.color_space())),
    };

    Ok(Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || {
        let bytes_per_component = (image.bits_per_component().as_u8() / 8) as u32;
        let color_channel_len = bytes_per_component
          * image.color_space().num_components() as u32
          * metadata.size.0
          * metadata.size.1;
        let color_channel = image.color_channel();
        assert_eq!(color_channel.len(), color_channel_len as usize);

        let alpha_channel_len = bytes_per_component * metadata.size.0 * metadata.size.1;
        let alpha_channel = image.alpha_channel();
        if let Some(alpha_channel) = alpha_channel {
          assert_eq!(alpha_channel.len(), alpha_channel_len as usize);
        }

        Ok(Repr::Sampled(SampledRepr {
          color_channel: deflate_encode(color_channel),
          alpha_channel: image.alpha_channel().map(deflate_encode),
          bits_per_component: image.bits_per_component(),
        }))
      }),
      metadata,
      sip: hash,
      interpolate,
    })))
  }

  /// Create a new RGB image from raw RGBA pixels.
  pub fn from_rgba8(data: Vec<u8>, width: u32, height: u32) -> Self {
    let hash = data.sip_hash();
    let metadata = ImageMetadata {
      has_alpha: true,
      size: (width, height),
      bits_per_component: BitsPerComponent::Eight,
      color_space: ImageColorspace::Rgb,
      icc: None,
    };

    Self(Arc::new(ImageRepr {
      inner: Deferred::new(move || {
        let (color_channel, alpha_channel, bits_per_component) =
          handle_u8_image(&data, ColorSpace::RGBA);

        Ok(Repr::Sampled(SampledRepr {
          color_channel,
          alpha_channel,
          bits_per_component,
        }))
      }),
      metadata,
      sip: hash,
      interpolate: false,
    }))
  }

  /// Return the size of the image.
  pub fn size(&self) -> (u32, u32) {
    self.0.size()
  }

  fn icc(&self) -> Option<GenericICCProfile> {
    self.0.icc()
  }

  fn color_space(&self) -> ImageColorspace {
    self.0.color_space()
  }

  pub(crate) fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let soft_mask_id = self.0.metadata.has_alpha.then(|| {
      sc.register_validation_error(ValidationError::Transparency(sc.location));
      sc.new_ref()
    });

    let icc_ref = self.icc().and_then(|ic| {
      if sc
        .serialize_settings()
        .pdf_version()
        .supports_icc(ic.metadata())
        && self.color_space().matches_icc_profile(&ic)
      {
        let ref_ = match ic {
          GenericICCProfile::Luma(l) => {
            sc.register_cacheable(chunk_container, ICCBasedColorSpace(l))
          }
          GenericICCProfile::Rgb(r) => {
            sc.register_cacheable(chunk_container, ICCBasedColorSpace(r))
          }
          GenericICCProfile::Cmyk(c) => {
            sc.register_cacheable(chunk_container, ICCBasedColorSpace(c))
          }
        };

        Some(ref_)
      } else {
        // Don't embed ICC profiles from images if the current
        // PDF version does not support it.
        None
      }
    });

    if self.0.interpolate {
      sc.register_validation_error(ValidationError::ImageInterpolation(sc.location));
    }

    let serialize_settings = sc.serialize_settings().clone();

    let cs = {
      let cs = match self.color_space() {
        ImageColorspace::Rgb => rgb::color_space(sc.serialize_settings().no_device_cs).into(),
        ImageColorspace::Luma => luma::color_space(sc.serialize_settings().no_device_cs).into(),
        ImageColorspace::Cmyk => match cmyk::color_space(&sc.serialize_settings()) {
          None => {
            sc.register_validation_error(ValidationError::MissingCMYKProfile);
            crate::krilla::color::DeviceColorSpace::Cmyk.into()
          }
          Some(cs) => cs.into(),
        },
      };

      sc.register_colorspace(chunk_container, cs)
    };

    let supports_bit_depth = sc
      .serialize_settings()
      .configuration
      .version()
      .supports_bit_depth(self.0.metadata.bits_per_component);
    let location = sc.location;
    let mut chunk = sc.new_chunk();

    let chunk = Deferred::new(move || {
      if !supports_bit_depth {
        return Err(KrillaError::SixteenBitImage(self.clone(), location));
      }

      let repr = self
        .0
        .inner
        .wait()
        .as_ref()
        .map_err(|e| KrillaError::Image(self.clone(), location, e.clone()))?;

      let alpha_mask = match repr {
        Repr::Sampled(sampled) => sampled.alpha_channel.as_ref().map(|mask_data| {
          let soft_mask_id = soft_mask_id.unwrap();
          let mask_stream =
            FilterStreamBuilder::new_from_deflated(mask_data).finish(&serialize_settings);
          let mut s_mask = chunk.image_xobject(soft_mask_id, mask_stream.encoded_data());
          mask_stream.write_filters(s_mask.deref_mut().deref_mut());
          s_mask.width(self.size().0 as i32);
          s_mask.height(self.size().1 as i32);
          s_mask.pair(
            Name(b"ColorSpace"),
            // Mask color space must be device gray -- see Table 145.
            DEVICE_GRAY.to_pdf_name(),
          );

          if self.0.interpolate {
            s_mask.interpolate(true);
          }

          s_mask.bits_per_component(repr.bits_per_component() as i32);
          soft_mask_id
        }),
        Repr::Jpeg(_) | Repr::Png(_) => None,
      };

      let filter_stream = match repr {
        Repr::Sampled(s) => {
          FilterStreamBuilder::new_from_deflated(&s.color_channel).finish(&serialize_settings)
        }
        Repr::Jpeg(j) => {
          FilterStreamBuilder::new_from_jpeg_data(j.data.as_ref()).finish(&serialize_settings)
        }
        Repr::Png(p) => FilterStreamBuilder::new_from_png_data(
          p.data.as_ref(),
          p.color_type.samples() as i32,
          p.bit_depth as i32,
          self.size().0 as i32,
        )
        .finish(&serialize_settings),
      };

      let mut image_x_object = chunk.image_xobject(root_ref, filter_stream.encoded_data());
      filter_stream.write_filters(image_x_object.deref_mut().deref_mut());
      image_x_object.width(self.size().0 as i32);
      image_x_object.height(self.size().1 as i32);

      if let Repr::Png(PngRepr {
        color_type: ColorType::Indexed,
        palette: Some(palette),
        ..
      }) = repr
      {
        set_indexed_colorspace(icc_ref, cs, palette, image_x_object.deref_mut());
      } else if let Some(icc_ref) = icc_ref {
        image_x_object.pair(Name(b"ColorSpace"), icc_ref);
      } else {
        cs.write(image_x_object.deref_mut().insert(Name(b"ColorSpace")));
      }

      if self.0.interpolate {
        image_x_object.interpolate(true);
      }

      // Photoshop CMYK images need to be inverted, see
      // https://github.com/sile-typesetter/libtexpdf/blob/1891bee5e0b73165e4a259f910d3ea3fe1df0b42/jpegimage.c#L25-L51
      // I'm not sure if this applies to all JPEG CMYK images out there, but for now we just
      // always do it. In libtexpdf, they only seem to do it if they can find the Adobe APP
      // marker.
      if let Repr::Jpeg(j) = repr {
        if j.invert_cmyk {
          image_x_object.decode([1.0, 0.0].repeat(self.color_space().num_components() as usize));
        }
      }

      image_x_object.bits_per_component(repr.bits_per_component() as i32);
      if let Some(soft_mask_id) = alpha_mask {
        image_x_object.s_mask(soft_mask_id);
      }
      image_x_object.finish();

      Ok(chunk)
    });

    chunk_container.streams.images.push(chunk);
  }
}

const PNG_TRANSFORMATIONS: Transformations = Transformations::EXPAND;

fn png_metadata(data: &[u8]) -> Result<ImageMetadata, String> {
  let cursor = Cursor::new(data);
  let mut decoder = ::png::Decoder::new(cursor);
  decoder.set_transformations(PNG_TRANSFORMATIONS);
  let reader = decoder
    .read_info()
    .map_err(|e| e.to_string().to_ascii_lowercase())?;
  let info = reader.info();

  let size = (info.width, info.height);
  let (color_type, bit_depth) = reader.output_color_type();
  let bits_per_component = match bit_depth {
    // We will normalize to 8 when decoding.
    BitDepth::One | BitDepth::Two | BitDepth::Four | BitDepth::Eight => BitsPerComponent::Eight,
    BitDepth::Sixteen => BitsPerComponent::Sixteen,
  };

  let (image_color_space, has_alpha) = match color_type {
    ColorType::Grayscale => (ImageColorspace::Luma, false),
    ColorType::GrayscaleAlpha => (ImageColorspace::Luma, true),
    ColorType::Rgb => (ImageColorspace::Rgb, false),
    ColorType::Rgba => (ImageColorspace::Rgb, true),
    ColorType::Indexed => {
      return Err("image uses an indexed color space, which is unsupported".to_string());
    }
  };
  let icc = info
    .icc_profile
    .as_ref()
    .and_then(|i| get_icc_profile_type(i, image_color_space));

  Ok(ImageMetadata {
    has_alpha,
    size,
    bits_per_component,
    color_space: image_color_space,
    icc,
  })
}

fn decode_png(data: &[u8]) -> Result<Repr, String> {
  let cursor = Cursor::new(data);
  let mut decoder = ::png::Decoder::new(cursor);
  decoder.set_transformations(PNG_TRANSFORMATIONS);
  let mut reader = decoder
    .read_info()
    .map_err(|e| e.to_string().to_ascii_lowercase())?;

  if let Some(png_data) = png::PngData::new(data) {
    return Ok(Repr::Png(PngRepr {
      data: png_data.idat,
      bit_depth: png_data.bit_depth,
      color_type: png_data.color_type,
      palette: png_data.palette,
    }));
  }

  let mut img_data = vec![0; reader.output_buffer_size().ok_or("image is too large")?];
  let _ = reader
    .next_frame(&mut img_data)
    .map_err(|e| e.to_string())?;
  let (color_type, bit_depth) = reader.output_color_type();

  let color_space = match color_type {
    ColorType::Rgb => ColorSpace::RGB,
    ColorType::Rgba => ColorSpace::RGBA,
    ColorType::Grayscale => ColorSpace::Luma,
    ColorType::GrayscaleAlpha => ColorSpace::LumaA,
    _ => unreachable!(),
  };

  let (color_channel, alpha_channel, bits_per_component) = match bit_depth {
    BitDepth::Eight => handle_u8_image(&img_data, color_space),
    BitDepth::Sixteen => handle_u16_image(&img_data, color_space),
    _ => return Err("image has an unsupported bit-depth".to_string()),
  };

  Ok(Repr::Sampled(SampledRepr {
    color_channel,
    alpha_channel,
    bits_per_component,
  }))
}

fn set_indexed_colorspace(
  icc_ref: Option<Ref>,
  base: MaybeDeviceColorSpace,
  palette: &[u8],
  target: &mut Dict,
) {
  let mut color_space = target.insert(Name(b"ColorSpace")).array();
  color_space.item(Name(b"Indexed"));

  if let Some(icc_ref) = icc_ref {
    color_space.item(icc_ref);
  } else {
    base.write(color_space.push());
  }

  color_space.item((palette.len() / 3 - 1) as i32);
  color_space.item(Str(palette));
  color_space.finish();
}

fn jpeg_metadata(data: &[u8]) -> Result<ImageMetadata, String> {
  let reader = Cursor::new(data);
  let mut decoder = JpegDecoder::new(reader);
  decoder
    .decode_headers()
    .map_err(|e| e.to_string().to_ascii_lowercase())?;

  let size = {
    let dimensions = decoder
      .dimensions()
      .ok_or("failed to read image dimensions".to_string())?;
    (dimensions.0 as u32, dimensions.1 as u32)
  };

  let image_color_space = decoder
    .input_colorspace()
    .and_then(|c| c.try_into().ok())
    .ok_or("failed to read image colorspace".to_string())?;

  let icc = decoder
    .icc_profile()
    .and_then(|d| get_icc_profile_type(&d, image_color_space));

  Ok(ImageMetadata {
    has_alpha: false,
    size,
    bits_per_component: BitsPerComponent::Eight,
    color_space: image_color_space,
    icc,
  })
}

fn decode_jpeg(data: Data) -> Result<Repr, String> {
  let reader = Cursor::new(data.as_ref());
  let mut decoder = JpegDecoder::new(reader);
  decoder
    .decode_headers()
    .map_err(|e| e.to_string().to_ascii_lowercase())?;

  let input_color_space = decoder
    .input_colorspace()
    .ok_or("failed to read image colorspace".to_string())?;

  if matches!(
    input_color_space,
    ColorSpace::Luma | ColorSpace::YCbCr | ColorSpace::RGB | ColorSpace::CMYK | ColorSpace::YCCK
  ) {
    Ok(Repr::Jpeg(JpegRepr {
      data,
      bits_per_component: BitsPerComponent::Eight,
      invert_cmyk: matches!(input_color_space, ColorSpace::YCCK | ColorSpace::CMYK),
    }))
  } else {
    // JPEGs shouldn't be able to have a different color space?
    Err("image has an unknown color space".to_string())
  }
}

fn decode_gif(data: Data) -> Result<Repr, String> {
  let mut decoder = gif::DecodeOptions::new();
  decoder.set_color_output(gif::ColorOutput::RGBA);
  let mut decoder = decoder
    .read_info(data.as_ref())
    .map_err(|e| e.to_string().to_ascii_lowercase())?;
  let first_frame = decoder
    .read_next_frame()
    .map_err(|e| e.to_string())?
    .ok_or("GIF image seems to be empty".to_string())?;

  let (color_channel, alpha_channel, bits_per_component) =
    handle_u8_image(first_frame.buffer.as_ref(), ColorSpace::RGBA);

  Ok(Repr::Sampled(SampledRepr {
    color_channel,
    alpha_channel,
    bits_per_component,
  }))
}

fn gif_metadata(data: &[u8]) -> Result<ImageMetadata, String> {
  let size = imagesize::blob_size(data).map_err(|e| e.to_string().to_ascii_lowercase())?;

  Ok(ImageMetadata {
    // We always decode GIFs using RGBA, see `decode_gif`
    has_alpha: true,
    bits_per_component: BitsPerComponent::Eight,
    size: (size.width as u32, size.height as u32),
    color_space: ImageColorspace::Rgb,
    icc: None,
  })
}

fn webp_metadata(data: &[u8]) -> Result<ImageMetadata, String> {
  let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(data))
    .map_err(|e| e.to_string().to_ascii_lowercase())?;
  let size = decoder.dimensions();
  let color_space = ImageColorspace::Rgb;
  let icc = decoder
    .icc_profile()
    .map_err(|e| e.to_string().to_ascii_lowercase())?
    .and_then(|d| get_icc_profile_type(&d, color_space));

  Ok(ImageMetadata {
    has_alpha: decoder.has_alpha(),
    bits_per_component: BitsPerComponent::Eight,
    size,
    color_space,
    icc,
  })
}

fn decode_webp(data: Data) -> Result<Repr, String> {
  let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(data.as_ref()))
    .map_err(|e| e.to_string().to_ascii_lowercase())?;
  let mut first_frame = vec![0; decoder.output_buffer_size().ok_or("image is too large")?];
  decoder
    .read_image(&mut first_frame)
    .map_err(|e| e.to_string().to_ascii_lowercase())?;

  let color_space = if decoder.has_alpha() {
    ColorSpace::RGBA
  } else {
    ColorSpace::RGB
  };

  let (color_channel, alpha_channel, bits_per_component) =
    handle_u8_image(&first_frame, color_space);

  Ok(Repr::Sampled(SampledRepr {
    color_channel,
    alpha_channel,
    bits_per_component,
  }))
}

fn handle_u8_image(data: &[u8], cs: ColorSpace) -> (Vec<u8>, Option<Vec<u8>>, BitsPerComponent) {
  let mut alphas = if cs.has_alpha() {
    Vec::with_capacity(data.len() / cs.num_components())
  } else {
    Vec::new()
  };

  let color_channel = match cs {
    ColorSpace::RGB => deflate_encode(data),
    ColorSpace::RGBA => {
      let mut buf = Vec::with_capacity(data.len() * 3 / 4);
      data.chunks_exact(4).for_each(|data| {
        buf.extend_from_slice(&data[0..3]);
        alphas.push(data[3]);
      });
      deflate_encode(&buf)
    }
    ColorSpace::Luma => deflate_encode(data),
    ColorSpace::LumaA => {
      let mut buf = Vec::with_capacity(data.len() / 2);
      data.chunks_exact(2).for_each(|data| {
        buf.push(data[0]);
        alphas.push(data[1]);
      });
      deflate_encode(&buf)
    }
    // PNG/WEBP/GIF only support those three, so should be enough?
    _ => unimplemented!(),
  };

  let alpha_channel = if !alphas.is_empty() && alphas.iter().any(|v| *v != 255) {
    Some(deflate_encode(&alphas))
  } else {
    None
  };

  (color_channel, alpha_channel, BitsPerComponent::Eight)
}

fn handle_u16_image(data: &[u8], cs: ColorSpace) -> (Vec<u8>, Option<Vec<u8>>, BitsPerComponent) {
  let mut alphas = if cs.has_alpha() {
    Vec::with_capacity(data.len() / cs.num_components())
  } else {
    Vec::new()
  };

  let encoded_image = match cs {
    ColorSpace::RGB => deflate_encode(data),
    ColorSpace::RGBA => {
      let mut buf = Vec::with_capacity(data.len() * 3 / 4);
      data.chunks_exact(8).for_each(|data| {
        buf.extend_from_slice(&data[0..6]);
        alphas.extend_from_slice(&data[6..]);
      });
      deflate_encode(&buf)
    }
    ColorSpace::Luma => deflate_encode(data),
    ColorSpace::LumaA => {
      let mut buf = Vec::with_capacity(data.len() / 2);
      data.chunks_exact(4).for_each(|data| {
        buf.extend_from_slice(&data[0..2]);
        alphas.extend_from_slice(&data[2..]);
      });
      deflate_encode(&buf)
    }
    // PNG/WEBP/GIF only support those three, so should be enough?
    _ => unimplemented!(),
  };

  let encoded_mask = if !alphas.is_empty() {
    Some(deflate_encode(&alphas))
  } else {
    None
  };

  (encoded_image, encoded_mask, BitsPerComponent::Sixteen)
}

fn get_icc_profile_type(data: &[u8], color_space: ImageColorspace) -> Option<GenericICCProfile> {
  let wrapper = match color_space {
    ImageColorspace::Rgb => GenericICCProfile::Rgb(ICCProfile::new(data)?),
    ImageColorspace::Luma => GenericICCProfile::Luma(ICCProfile::new(data)?),
    ImageColorspace::Cmyk => GenericICCProfile::Cmyk(ICCProfile::new(data)?),
  };

  Some(wrapper)
}

#[cfg(test)]
mod tests {
  use crate::krilla::image::Image;
  use std::sync::Arc;

  #[test]
  fn invalid_png_image() {
    let e = Image::from_png(Arc::new(b"dfngiudfg".to_vec()).into(), false);
    assert!(e.is_err());
  }

  #[test]
  fn invalid_jpeg_image() {
    let e = Image::from_jpeg(Arc::new(b"dfngiudfg".to_vec()).into(), false);
    assert!(e.is_err());
  }
}
