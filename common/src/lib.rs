pub const TCP_PACKET_SIZE: usize = 2_usize.pow(16);

pub fn human_readable_time(nanos: f64) -> String {
  const SUBDIVISION: f64 = 1000.0;
  const UNITS: [&str; 4] = ["ns", "μs", "ms", "s"];

  to_human_readable(nanos, SUBDIVISION, &UNITS)
}

pub fn human_readable_data(bytes: f64) -> String {
  const SUBDIVISION: f64 = 1024.0;
  const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

  to_human_readable(bytes, SUBDIVISION, &UNITS)
}

fn to_human_readable(mut value: f64, subdivision: f64, units: &[&str]) -> String {
  let mut divisions = 0;
  while (value >= subdivision) && (divisions <= units.len()) {
    value /= subdivision;
    divisions += 1;
  }

  format!("{:.2}{}", value, units[divisions])
}
