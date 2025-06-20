use std::{
  num::NonZeroUsize,
  path::Path,
  sync::{Arc, Mutex},
};

use async_trait::async_trait;
use cosmic_text::{FontSystem, SwashCache};

use crate::{
  font::{FontError, load_woff2_font},
  node::draw::ImageState,
};

/// A trait for storing and retrieving images in an image rendering system.
///
/// This trait allows implementors to provide their own image storage and caching mechanisms.
/// The trait is designed to be thread-safe and can be used in async contexts.
///
/// # Example
/// ```rust
/// use std::sync::Arc;
/// use takumi::context::ImageStore;
/// use takumi::node::draw::ImageState;
/// use async_trait::async_trait;
///
/// #[derive(Debug)]
/// struct MyImageStore {
///   // http client and image store hashmap
/// }
///
/// #[async_trait]
/// impl ImageStore for MyImageStore {
///     fn get(&self, key: &str) -> Option<Arc<ImageState>> {
///         // Implement image retrieval
///         None
///     }
///
///     fn insert(&self, key: String, value: Arc<ImageState>) {
///         // Implement image storage
///     }
///
///     async fn fetch_async(&self, key: &str) -> Arc<ImageState> {
///         // Implement async image fetching
///         unimplemented!()
///     }
/// }
/// ```
#[async_trait]
pub trait ImageStore: Send + Sync + std::fmt::Debug {
  /// Retrieves an image from the store by its key.
  fn get(&self, key: &str) -> Option<Arc<ImageState>>;

  /// Stores an image in the store with the given key.
  fn insert(&self, key: String, value: Arc<ImageState>);

  /// Asynchronously fetches an image from a remote source and stores it.
  async fn fetch_async(&self, key: &str) -> Arc<ImageState>;
}

/// A context for managing fonts in the rendering system.
///
/// This struct holds the font system and cache used for text rendering.
#[derive(Debug)]
pub struct FontContext {
  /// The font system used for text layout and rendering
  pub font_system: Mutex<FontSystem>,
  /// The cache for font glyphs and metrics
  pub font_cache: Mutex<SwashCache>,
}

/// The main context for image rendering.
///
/// This struct holds all the necessary state for rendering images, including
/// font management, image storage, and debug options.
#[derive(Debug)]
pub struct GlobalContext {
  /// Whether to print the debug tree during layout
  pub print_debug_tree: bool,
  /// Whether to draw debug borders around nodes
  pub draw_debug_border: bool,
  /// The font context for text rendering
  pub font_context: FontContext,
  /// The image store for caching and retrieving images
  pub image_store: Box<dyn ImageStore>,
}

impl Default for GlobalContext {
  fn default() -> Self {
    Self {
      print_debug_tree: false,
      draw_debug_border: false,
      font_context: FontContext {
        font_system: Mutex::new(FontSystem::new()),
        font_cache: Mutex::new(SwashCache::new()),
      },
      #[cfg(feature = "default_impl")]
      image_store: Box::new(DefaultImageStore::new(
        Client::new(),
        NonZeroUsize::new(100).unwrap(),
      )),
      #[cfg(not(feature = "default_impl"))]
      image_store: Box::new(NoopImageStore),
    }
  }
}

#[cfg(feature = "default_impl")]
use lru::LruCache;

#[cfg(feature = "default_impl")]
use reqwest::Client;

#[cfg(feature = "default_impl")]
/// A default implementation of ImageStore that uses a LRU cache and a HTTP client.
#[derive(Debug)]
pub struct DefaultImageStore {
  store: Mutex<LruCache<String, Arc<ImageState>>>,
  http: Client,
}

#[cfg(feature = "default_impl")]
impl DefaultImageStore {
  /// Creates a new DefaultImageStore with the given HTTP client and maximum size.
  pub fn new(http: Client, max_size: NonZeroUsize) -> Self {
    Self {
      store: Mutex::new(LruCache::new(max_size)),
      http,
    }
  }
}

#[cfg(feature = "default_impl")]
#[async_trait]
impl ImageStore for DefaultImageStore {
  fn get(&self, key: &str) -> Option<Arc<ImageState>> {
    self.store.lock().unwrap().get(key).cloned()
  }

  fn insert(&self, key: String, value: Arc<ImageState>) {
    self.store.lock().unwrap().put(key, value);
  }

  async fn fetch_async(&self, key: &str) -> Arc<ImageState> {
    let Ok(response) = self.http.get(key).send().await else {
      return Arc::new(ImageState::NetworkError);
    };

    let Ok(body) = response.bytes().await else {
      return Arc::new(ImageState::NetworkError);
    };

    let image = image::load_from_memory(body.as_ref());

    if let Err(e) = image {
      return Arc::new(ImageState::DecodeError(e));
    }

    Arc::new(ImageState::Fetched(image.unwrap().into()))
  }
}

/// A no-op implementation of ImageStore that does nothing.
///
/// This is used as the default implementation when no custom ImageStore is provided.
/// It always returns None for get operations and does nothing for insert operations.
#[derive(Default, Debug)]
pub struct NoopImageStore;

#[async_trait]
impl ImageStore for NoopImageStore {
  /// Always returns None as this is a no-op implementation.
  fn get(&self, _key: &str) -> Option<Arc<ImageState>> {
    None
  }

  /// Does nothing as this is a no-op implementation.
  fn insert(&self, _key: String, _value: Arc<ImageState>) {
    // No-op
  }

  /// Always panics as this is a no-op implementation.
  async fn fetch_async(&self, _key: &str) -> Arc<ImageState> {
    unimplemented!("NoopImageStore does not support fetching images")
  }
}

/// Loads a WOFF2 font file and adds it to the font context.
pub fn load_woff2_font_to_context(
  font_context: &FontContext,
  font_file: &Path,
) -> Result<(), FontError> {
  let font = load_woff2_font(font_file)?;
  let mut system = font_context.font_system.lock().unwrap();
  system.db_mut().load_font_data(font);
  Ok(())
}
