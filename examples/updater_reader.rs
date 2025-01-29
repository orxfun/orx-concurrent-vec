use clap::Parser;
use orx_concurrent_vec::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

fn fmt(n: usize) -> String {
    n.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(",")
}

/// Runs the reader & updater scenario using the lock-free ConcurrentVec<u64>.
fn run_with_con_vec(len: usize, num_readers: usize, num_updaters: usize, duration_seconds: u128) {
    fn update(vec: &ConcurrentVec<u64>, seed: u64, duration_seconds: u128) -> usize {
        let mut num_updates = 0;
        let mut r = ChaCha8Rng::seed_from_u64(seed);

        let instant = Instant::now();
        while instant.elapsed().as_millis() < duration_seconds * 1000 {
            let i = r.random_range(1..vec.len());
            vec[i].set((i as u64 + 1) % 1000);
            num_updates += 1;
        }

        num_updates
    }

    fn read(vec: &ConcurrentVec<u64>, seed: u64, duration_seconds: u128) -> usize {
        let mut num_reads = 0;
        let mut r = ChaCha8Rng::seed_from_u64(seed);

        let len = vec.len();
        let instant = Instant::now();
        while instant.elapsed().as_millis() < duration_seconds * 1000 {
            let i = r.random_range(1..len);
            let value = vec.get_cloned(i).unwrap();
            assert!(value < len as u64);

            num_reads += 1;
        }

        num_reads
    }

    let vec = ConcurrentVec::new();
    vec.extend((0..len).map(|i| i as u64));

    let (num_updates, num_reads) = std::thread::scope(|s| {
        let vec = &vec;

        let mut update_handles = vec![];
        for i in 0..num_updaters {
            update_handles.push(s.spawn(move || update(&vec, i as u64, duration_seconds)));
        }

        let mut read_handles = vec![];
        for i in 0..num_readers {
            read_handles.push(s.spawn(move || read(&vec, i as u64, duration_seconds)));
        }

        let mut num_reads = 0;
        for x in read_handles {
            num_reads += x.join().unwrap();
        }

        let mut num_updates = 0;
        for x in update_handles {
            num_updates += x.join().unwrap();
        }

        (num_updates, num_reads)
    });

    println!(
        "\nConcurrentVec<u64>\n* num-updates:\t{}\n* num-reads:\t{}\n* total-ops:\t{}\n",
        fmt(num_updates),
        fmt(num_reads),
        fmt(num_reads + num_updates),
    );
}

/// Runs the reader & updater scenario using the locking Arc<Mutex<Vec<u64>>>.
fn run_with_arc_mutex_vec(
    len: usize,
    num_readers: usize,
    num_updaters: usize,
    duration_seconds: u128,
) {
    fn update(vec: Arc<Mutex<Vec<u64>>>, seed: u64, duration_seconds: u128) -> usize {
        let mut num_updates = 0;
        let vec_len = vec.lock().unwrap().len();
        let mut r = ChaCha8Rng::seed_from_u64(seed);

        let instant = Instant::now();
        while instant.elapsed().as_millis() < duration_seconds * 1000 {
            let i = r.random_range(1..vec_len);
            vec.lock().unwrap()[i] = (i as u64 + 1) % 1000;
            num_updates += 1;
        }
        num_updates
    }

    fn read(vec: Arc<Mutex<Vec<u64>>>, seed: u64, duration_seconds: u128) -> usize {
        let mut num_reads = 0;
        let mut r = ChaCha8Rng::seed_from_u64(seed);

        let len = vec.lock().unwrap().len();
        let instant = Instant::now();
        while instant.elapsed().as_millis() < duration_seconds * 1000 {
            let i = r.random_range(1..len);
            let value = vec.lock().unwrap()[i];
            assert!(value < len as u64);

            num_reads += 1;
        }

        num_reads
    }

    let vec: Vec<_> = (0..len).map(|i| i as u64).collect();
    let vec = Arc::new(Mutex::new(vec));

    let (num_updates, num_reads) = std::thread::scope(|s| {
        let vec = &vec;

        let mut update_handles = vec![];
        for i in 0..num_updaters {
            update_handles.push(s.spawn(move || update(vec.clone(), i as u64, duration_seconds)));
        }

        let mut read_handles = vec![];
        for i in 0..num_readers {
            read_handles.push(s.spawn(move || read(vec.clone(), i as u64, duration_seconds)));
        }

        let mut num_reads = 0;
        for x in read_handles {
            num_reads += x.join().unwrap();
        }

        let mut num_updates = 0;
        for x in update_handles {
            num_updates += x.join().unwrap();
        }

        (num_updates, num_reads)
    });

    println!(
        "\nArc<Mutex<Vec<u64>>>\n* num-updates:\t{}\n* num-reads:\t{}\n* total-ops:\t{}\n",
        fmt(num_updates),
        fmt(num_reads),
        fmt(num_reads + num_updates),
    );
}

/// Program to test and demonstrate the impact of locking and lock-free
/// approaches to update and read from a vector.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Final length of the vector to read and update concurrently.
    #[arg(long, default_value_t = 10_000)]
    len: usize,

    /// Number of threads that will continuously read elements.
    #[arg(long, default_value_t = 8)]
    num_readers: usize,

    /// Number of threads that will continuously update elements.
    #[arg(long, default_value_t = 8)]
    num_updaters: usize,

    /// Total duration of the experiment.
    #[arg(long, default_value_t = 4)]
    duration_seconds: u128,
}

/// Program to test and demonstrate the impact of locking and lock-free
/// approaches to update and read from a vector.
fn main() {
    let args = Args::parse();

    println!("\nNumber of read & update operations that can be achieved within a fixed duration.");
    println!("\n{:?}", args);

    let (len, num_readers, num_updaters, duration_seconds) = (
        args.len,
        args.num_readers,
        args.num_updaters,
        args.duration_seconds,
    );

    assert!(len > 0);
    assert!(num_readers > 0);
    assert!(num_updaters > 0);
    assert!(duration_seconds > 0);

    run_with_arc_mutex_vec(len, num_readers, num_updaters, duration_seconds);
    run_with_con_vec(len, num_readers, num_updaters, duration_seconds);
}
