use clap::Parser;
use common::TCP_PACKET_SIZE;
use rand::Rng;
use std::cmp;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};
use tracing::{info, Level};

#[derive(Parser, Debug)]
struct Args {
  /// IP address to send data to
  #[arg(short, long, default_value = "127.0.0.1")]
  address: String,

  /// Port to send data to
  #[arg(short, long)]
  port: u16,

  /// Url to send data to
  #[arg(short, long, default_value = "/")]
  url: String,
}

fn main() -> Result<()> {
  tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .with_max_level(Level::DEBUG)
    .init();

  let args = Args::parse();

  benchmark(&args)?;

  Ok(())
}

#[tracing::instrument(skip_all)]
fn benchmark(args: &Args) -> Result<()> {
  const DATA_SIZE: usize = 100 * 1024 * 1024;
  const WARMUP_ITERATIONS: usize = 40;
  const BENCHMARK_ITERATIONS: usize = 100;

  let mut buf = [0; TCP_PACKET_SIZE];
  fill(&mut buf);

  info!("benchmarking {}:{}{}", args.address, args.port, args.url);

  for iteration in 1..=WARMUP_ITERATIONS {
    info!("[{} / {}] warming up", iteration, WARMUP_ITERATIONS);
    send_and_measure(args, DATA_SIZE, &buf)?;
  }

  let mut sum = Duration::ZERO;
  for iteration in 1..=BENCHMARK_ITERATIONS {
    let time = send_and_measure(args, DATA_SIZE, &buf)?;
    let time_nanos = time.as_nanos();
    info!(
      "[{} / {}] send {} took {} ({}/s)",
      iteration,
      BENCHMARK_ITERATIONS,
      common::human_readable_data(DATA_SIZE as f64),
      common::human_readable_time(time_nanos as f64),
      common::human_readable_data(bytes_per_second(DATA_SIZE, time_nanos))
    );

    sum += time;
  }

  let average = sum / (BENCHMARK_ITERATIONS as u32);
  info!(
    "{} sends, average response {} (total {} at {}/s)",
    BENCHMARK_ITERATIONS,
    common::human_readable_time(average.as_nanos() as f64),
    common::human_readable_data((DATA_SIZE * BENCHMARK_ITERATIONS) as f64),
    common::human_readable_data(bytes_per_second(DATA_SIZE, average.as_nanos()))
  );

  Ok(())
}

fn fill(buf: &mut [u8]) {
  let mut random = rand::thread_rng();
  for item in buf {
    *item = random.gen();
  }
}

fn send_and_measure(args: &Args, size: usize, buf: &[u8]) -> Result<Duration> {
  let host = format!("{}:{}", args.address, args.port);
  let stream = TcpStream::connect(&host)?;

  let start_time = Instant::now();
  send(&host, stream, &args.url, size, buf)?;
  let end_time = start_time.elapsed();

  Ok(end_time)
}

fn send(host: &str, mut stream: TcpStream, url: &str, size: usize, buf: &[u8]) -> Result<()> {
  stream.write_all(
    format!(
      "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\n\r\n",
      url, host, size
    )
    .as_bytes(),
  )?;

  let mut sent = 0;
  while sent < size {
    let remaining = cmp::min(buf.len(), size - sent);
    stream.write_all(&buf[..remaining])?;
    sent += remaining;
  }

  const EXPECTED_RESPONSE: &str = "HTTP/1.1";
  let mut response_buf = [0; EXPECTED_RESPONSE.len()];
  stream.read_exact(&mut response_buf)?;
  let response = String::from_utf8_lossy(&response_buf);

  if response != EXPECTED_RESPONSE {
    return Err(Error::new(
      ErrorKind::Unsupported,
      format!(
        "expected response to begin with {} but got {}",
        EXPECTED_RESPONSE, response
      ),
    ));
  }

  Ok(())
}

fn bytes_per_second(data_bytes: usize, time_nanos: u128) -> f64 {
  data_bytes as f64 / ((time_nanos as f64) / (1000.0 * 1000.0 * 1000.0))
}
