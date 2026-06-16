use std::time::Duration;

use criterion::Criterion;

pub fn criterion() -> Criterion {
  Criterion::default()
    .warm_up_time(Duration::from_millis(500))
    .measurement_time(Duration::from_secs(2))
    .sample_size(20)
}
