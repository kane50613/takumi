use clap::Parser;
use mimalloc::MiMalloc;
use takumi::base::GlobalContext;
use tracing::Level;
use tracing_subscriber::fmt;

use takumi_server::{Args, run_server};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
  fmt().with_max_level(Level::INFO).init();

  let args = Args::parse();

  let context = GlobalContext::default();

  run_server(args, context).await;
}
