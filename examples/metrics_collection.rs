use orx_concurrent_vec::*;
use std::time::Duration;

#[derive(Debug, Default)]
struct Metric {
    sum: i32,
    count: i32,
}

impl Metric {
    fn aggregate(self, value: &i32) -> Self {
        Self {
            sum: self.sum + value,
            count: self.count + 1,
        }
    }
}

fn main() {
    // record measurements in random intervals, roughly every 2ms
    let measurements = ConcurrentVec::new();

    // collect metrics every 100 milliseconds
    let metrics = ConcurrentVec::new();

    std::thread::scope(|s| {
        // thread to store measurements as they arrive
        s.spawn(|| {
            for i in 0..100 {
                std::thread::sleep(Duration::from_millis(i % 5));

                // collect measurements and push to measurements vec
                measurements.push(i as i32);
            }
        });

        // thread to collect metrics every 100 milliseconds
        s.spawn(|| {
            for _ in 0..10 {
                // safely read from measurements vec to compute the metric at that instant
                let metric = measurements.fold(Metric::default(), |x, value| x.aggregate(value));

                // push result to metrics
                metrics.push(metric);

                std::thread::sleep(Duration::from_millis(100));
            }
        });
    });

    let measurements: Vec<_> = measurements.to_vec();
    let averages: Vec<_> = metrics.to_vec();

    assert_eq!(measurements.len(), 100);
    assert_eq!(averages.len(), 10);
}
