use async_tempfile::TempFile;
use axum::body::Body;
use axum::routing::post;
use axum::Router;
use clap::Parser;
use futures::TryStreamExt;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_util::io::StreamReader;
use tracing::{info, Level};

#[derive(Parser)]
struct Args {
  /// Interface address to bind on
  #[arg(short, long, default_value = "0.0.0.0")]
  interface: String,

  /// Port to bind on
  #[arg(short, long, default_value_t = 5001)]
  port: u16,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .with_max_level(Level::DEBUG)
    .init();

  let args = Args::parse();

  let bind = format!("{}:{}", args.interface, args.port);
  let listener = TcpListener::bind(&bind).await.expect("could not bind");
  info!("listening on {}", bind);

  let app = Router::new().route("/", post(ingest));

  axum::serve(listener, app).await.expect("could not serve");
}

async fn ingest(body: Body) {
  let mut file = TempFile::new()
    .await
    .expect("could not create temp file")
    .open_rw()
    .await
    .expect("could not open temp file");

  let mut reader = StreamReader::new(
    body
      .into_data_stream()
      .map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
  );

  io::copy(&mut reader, &mut file)
    .await
    .expect("could not copy to file");
  file.flush().await.expect("could not flush file");
}
