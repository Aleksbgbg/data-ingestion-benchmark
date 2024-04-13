use actix_web::web::{Bytes, PayloadConfig};
use actix_web::{App, HttpResponse, HttpServer, Responder};
use async_tempfile::TempFile;
use clap::Parser;
use std::io::Result;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tracing::{info, Level};

#[derive(Parser)]
struct Args {
  /// Interface address to bind on
  #[arg(short, long, default_value = "0.0.0.0")]
  interface: String,

  /// Port to bind on
  #[arg(short, long, default_value_t = 5002)]
  port: u16,
}

#[actix_web::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .with_max_level(Level::DEBUG)
    .init();

  let args = Args::parse();

  info!("listening on {}:{}", args.interface, args.port);
  HttpServer::new(|| {
    App::new()
      .service(ingest)
      .app_data(PayloadConfig::new(1024 * 1024 * 1024 * 1024)) // 1TiB
  })
  .bind((args.interface, args.port))?
  .run()
  .await
}

#[actix_web::post("/")]
async fn ingest(body: Bytes) -> impl Responder {
  let mut file = TempFile::new()
    .await
    .expect("could not create temp file")
    .open_rw()
    .await
    .expect("could not open temp file");

  let mut buf: &[u8] = &body;
  io::copy(&mut buf, &mut file)
    .await
    .expect("could not copy to file");
  file.flush().await.expect("could not flush file");

  HttpResponse::NoContent()
}
