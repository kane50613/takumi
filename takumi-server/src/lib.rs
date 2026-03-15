use std::{fs::read, net::SocketAddr, sync::Arc};

use axum::{Router, extract::State, http::StatusCode, response::Response, routing::get};
use globwalk::glob;
use takumi::GlobalContext;
use tokio::net::TcpListener;
use tracing::{error, info};

pub use crate::{args::Args, generate_image::*};

pub mod args;
pub mod generate_image;
#[cfg(feature = "hmac_verify")]
pub mod hmac_verify;

pub type AxumState = State<Arc<AxumStateInner>>;
pub type AxumResult<T = Response> = Result<T, (StatusCode, String)>;

pub struct AxumStateInner {
  pub context: GlobalContext,
  #[cfg(feature = "hmac_verify")]
  pub hmac_key: Option<Vec<u8>>,
}

pub fn create_state(args: Args, context: GlobalContext) -> AxumState {
  let state = Arc::new(AxumStateInner {
    context,
    #[cfg(feature = "hmac_verify")]
    hmac_key: args.hmac_key.map(|key| {
      use sha2::{Digest, Sha256};

      let mut hasher = Sha256::new();
      hasher.update(key.as_bytes());
      hasher.finalize().to_vec()
    }),
  });

  State(state)
}

pub fn create_app(state: AxumState) -> Router {
  let mut app = Router::new()
    .route("/image", get(generate_image_handler))
    .with_state(state.0.clone());

  #[cfg(feature = "hmac_verify")]
  if state.hmac_key.is_some() {
    app = app.layer(axum::middleware::from_fn_with_state(
      state.0,
      hmac_verify::hmac_verify_middleware,
    ));
  }

  app
}

pub async fn run_server(args: Args, mut context: GlobalContext) {
  if let Some(font_glob) = args.font_glob.as_ref() {
    for font in glob(font_glob).unwrap() {
      match font {
        Ok(path) => {
          if path.path().is_dir() {
            continue;
          }

          let file = read(path.path()).unwrap();

          if let Err(e) = context
            .font_context_mut()
            .load_and_store(file.into(), None, None)
          {
            error!("Failed to load font {}: {e:?}", path.file_name().display());
            continue;
          }

          info!("Loaded font: {}", path.file_name().display())
        }
        Err(e) => error!("Failed to load font: {e:?}"),
      }
    }
  }

  let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
  let listener = TcpListener::bind(addr).await.unwrap();

  info!("Image generator server running on http://{addr}");

  axum::serve(listener, create_app(create_state(args, context)))
    .await
    .unwrap();
}
