use async_tempfile::TempFile;
use clap::Parser;
use common::TCP_PACKET_SIZE;
use std::cmp;
use std::io::{Error, ErrorKind};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Result};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, Level};

#[derive(Parser)]
struct Args {
  /// Interface address to bind on
  #[arg(short, long, default_value = "0.0.0.0")]
  interface: String,

  /// Port to bind on
  #[arg(short, long, default_value_t = 5000)]
  port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .with_max_level(if cfg!(debug_assertions) {
      Level::DEBUG
    } else {
      Level::INFO
    })
    .init();

  let args = Args::parse();

  let bind = format!("{}:{}", args.interface, args.port);
  let listener = TcpListener::bind(&bind).await?;
  info!("listening on {}", bind);

  loop {
    let (stream, _) = listener.accept().await?;
    ingest(stream).await?;
  }
}

#[tracing::instrument(skip_all)]
async fn ingest(mut stream: TcpStream) -> Result<()> {
  const CONTENT_LENGTH_HEADER: &str = "content-length:";
  const NEWLINE_SEPARATOR: &str = "\r\n";
  const BODY_SEPARATOR: &str = "\r\n\r\n";

  debug!("begin");

  let start_time = Instant::now();

  let mut scanner = Scanner::new(&mut stream);

  scanner.consume_until(CONTENT_LENGTH_HEADER).await?;
  let length_bytes = scanner
    .read_until(NEWLINE_SEPARATOR)
    .await?
    .trim()
    .to_owned();
  let length_bytes: usize = length_bytes.parse().map_err(|err| {
    Error::new(
      ErrorKind::Unsupported,
      format!(
        "could not parse content length: {} (was '{}')",
        err, length_bytes
      ),
    )
  })?;
  debug!(
    "expecting body length of {}",
    common::human_readable_data(length_bytes as f64)
  );

  scanner.consume_until(BODY_SEPARATOR).await?;

  let mut file = TempFile::new()
    .await
    .expect("could not create temp file")
    .open_rw()
    .await
    .expect("could not open temp file");
  scanner.read_n_bytes_into(length_bytes, &mut file).await?;

  stream
    .write_all("HTTP/1.1 204 No Content\r\n\r\n".as_bytes())
    .await?;

  debug!(
    "end (server-side took {})",
    common::human_readable_time(start_time.elapsed().as_nanos() as f64)
  );

  Ok(())
}

/// A simple wrapper around [`TcpStream`] which searches for strings and
/// consumes bytes, reading from the stream as necessary. Note that this will
/// **not** search across packet boundaries! If this is required in the future
/// the wrapper should be adapted for large searches.
struct Scanner<'a> {
  stream: &'a mut TcpStream,
  buf: [u8; TCP_PACKET_SIZE],
  position: usize,
  size: usize,
}

impl<'a> Scanner<'a> {
  pub fn new(stream: &mut TcpStream) -> Scanner<'_> {
    Scanner {
      stream,
      buf: [0; TCP_PACKET_SIZE],
      position: 0,
      size: 0,
    }
  }

  /// Read until `needle`, and consume it.
  pub async fn consume_until(&mut self, needle: &str) -> Result<()> {
    loop {
      let eof = self.fill_and_check_eof().await?;
      if let Some(index) =
        contains_lowercase(&self.buf[self.position..self.size], needle.as_bytes())
      {
        self.position += index + needle.len();
        return Ok(());
      }

      if eof {
        return Err(Error::new(
          ErrorKind::Unsupported,
          format!("could not find: '{}' in request", needle),
        ));
      }
    }
  }

  /// Read until `needle`, but do **not** consume it.
  pub async fn read_until(&mut self, needle: &str) -> Result<String> {
    loop {
      let eof = self.fill_and_check_eof().await?;
      if let Some(index) =
        contains_lowercase(&self.buf[self.position..self.size], needle.as_bytes())
      {
        let content = String::from_utf8_lossy(&self.buf[self.position..self.position + index]);
        self.position += index;
        return Ok(content.into());
      }

      if eof {
        return Err(Error::new(
          ErrorKind::Unsupported,
          format!("could not find: '{}' in request", needle),
        ));
      }
    }
  }

  pub async fn read_n_bytes_into(
    &mut self,
    mut remaining: usize,
    sink: &mut TempFile,
  ) -> Result<()> {
    loop {
      let eof = self.fill_and_check_eof().await?;

      let must_read_bytes = cmp::min(self.available(), remaining);
      sink
        .write_all(&self.buf[self.position..self.position + must_read_bytes])
        .await?;

      self.position += must_read_bytes;
      remaining -= must_read_bytes;

      if remaining == 0 {
        sink.flush().await.expect("could not flush file");
        return Ok(());
      }

      if eof {
        return Err(Error::new(
          ErrorKind::Unsupported,
          format!(
            "not enough bytes in request (needed another {} bytes)",
            remaining
          ),
        ));
      }
    }
  }

  async fn fill_and_check_eof(&mut self) -> Result<bool> {
    if self.available() > 0 {
      return Ok(false);
    }

    self.position = 0;
    self.size = self.stream.read(&mut self.buf).await?;

    Ok(self.size == 0)
  }

  fn available(&self) -> usize {
    self.size - self.position
  }
}

fn contains_lowercase(buf: &[u8], pattern: &[u8]) -> Option<usize> {
  if buf.len() < pattern.len() {
    return None;
  }

  'outer: for buf_index in 0..=(buf.len() - pattern.len()) {
    for pattern_index in 0..pattern.len() {
      if buf[buf_index + pattern_index].to_ascii_lowercase() != pattern[pattern_index] {
        continue 'outer;
      }
    }

    return Some(buf_index);
  }

  None
}
