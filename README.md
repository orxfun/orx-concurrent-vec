# orx-concurrent-vec

[![orx-concurrent-vec crate](https://img.shields.io/crates/v/orx-concurrent-vec.svg)](https://crates.io/crates/orx-concurrent-vec)
[![orx-concurrent-vec documentation](https://docs.rs/orx-concurrent-vec/badge.svg)](https://docs.rs/orx-concurrent-vec)

An efficient, convenient and lightweight grow-only read & write concurrent data structure allowing high performance concurrent collection.

* **convenient**: `ConcurrentVec` can safely be shared among threads simply as a shared reference. It is a [`PinnedConcurrentCol`](https://crates.io/crates/orx-pinned-concurrent-col) with a special concurrent state implementation. Underlying [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) and concurrent bag can be converted back and forth to each other.
* **efficient**: `ConcurrentVec` is a lock free structure suitable for concurrent, copy-free and high performance growth.

## Examples

Underlying `PinnedVec` guarantees make it straightforward to safely grow with a shared reference which leads to a convenient api as demonstrated below.

The following example demonstrates use of two collections together:
* A `ConcurrentVec` is used to collect measurements taken in random intervals. ConcurrentVec is used since while collecting measurements, another thread will be reading them to compute statistics (read & write).
* A `ConcurrentBag` is used to collect statistics from the measurements at defined time intervals. ConcurrentBag is used since we do not need to read the statistics until the process completes (write-only).

```rust
use orx_concurrent_vec::*;
use orx_concurrent_bag::*;
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

    fn average(&self) -> i32 {
        match self.count {
            0 => 0,
            _ => self.sum / self.count,
        }
    }
}

// record measurements in random intervals, roughly every 2ms (read & write -> ConcurrentVec)
let measurements = ConcurrentVec::new();
let rf_measurements = &measurements; // just &self to share among threads

// collect metrics every 100 milliseconds (only write -> ConcurrentBag)
let metrics = ConcurrentBag::new();
let rf_metrics = &metrics; // just &self to share among threads

std::thread::scope(|s| {
    // thread to store measurements as they arrive
    s.spawn(move || {
        for i in 0..100 {
            std::thread::sleep(Duration::from_millis(i % 5));

            // collect measurements and push to measurements vec
            // simply by calling `push`
            rf_measurements.push(i as i32);
        }
    });

    // thread to collect metrics every 100 milliseconds
    s.spawn(move || {
        for _ in 0..10 {
            // safely read from measurements vec to compute the metric
            let metric = rf_measurements
                .iter()
                .fold(Metric::default(), |x, value| x.aggregate(value));

            // push result to metrics bag
            rf_metrics.push(metric);

            std::thread::sleep(Duration::from_millis(100));
        }
    });
});

let measurements: Vec<_> = measurements
    .into_inner()
    .into_iter()
    .map(|x| x.unwrap())
    .collect();
dbg!(&measurements);

let averages: Vec<_> = metrics
    .into_inner()
    .into_iter()
    .map(|x| x.average())
    .collect();
println!("averages = {:?}", &averages);

assert_eq!(measurements.len(), 100);
assert_eq!(averages.len(), 10);
```

## Properties of the Concurrent Model

ConcurrentVec wraps a [`PinnedVec`](https://crates.io/crates/orx-pinned-vex) of [`ConcurrentOption`](https://crates.io/crates/orx-concurrent-option) elements. This composition leads to the following safety guarantees:

* `ConcurrentOption` wrapper of elements allow for thread safe initialization while other threads can safely attempt to read the data. This is required for concurrent read and write operations.
* Underlying `PinnedVec` makes sure that memory location of pushed elements do not change. This allows for copy-free and safe concurrent growth.

Together, concurrency model of the `ConcurrentVec` has the following properties:

* Writing to a position of the collection does not block other writes, multiple writes can happen concurrently.
* Each position is written exactly once ⟹ no write & write race condition exists.
* Only one capacity growth can happen at a given time. Growth is copy-free and does not change memory locations of already pushed elements.
* Underlying pinned vector is always valid and can be taken out any time by `into_inner(self)`.
* Attempting to read from a position while its value is being written will safely yield `None`. If the element has been properly initialized, the other threads will safely read `Some(T)`.
* In other words, a position will be written exactly once, but can be read multiple times concurrently. ConcurrentVec prevents data races so that reading can only happen after it is completely written ⟹ no read & write race condition exists.

## Concurrent Friend Collections

||[`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag)|[`ConcurrentVec`](https://crates.io/crates/orx-concurrent-vec)|[`ConcurrentOrderedBag`](https://crates.io/crates/orx-concurrent-ordered-bag)|
|---|---|---|---|
| Write | Guarantees that each element is written exactly once via `push` or `extend` methods | Guarantees that each element is written exactly once via `push` or `extend` methods | Different in two ways. First, a position can be written multiple times. Second, an arbitrary element of the bag can be written at any time at any order using `set_value` and `set_values` methods. This provides a great flexibility while moving the safety responsibility to the caller; hence, the set methods are `unsafe`. |
| Read | Mainly, a write-only collection. Concurrent reading of already pushed elements is through `unsafe` `get` and `iter` methods. The caller is required to avoid race conditions. | A write-and-read collection. Already pushed elements can safely be read through `get` and `iter` methods. | Not supported currently. Due to the flexible but unsafe nature of write operations, it is difficult to provide required safety guarantees as a caller. |
| Ordering of Elements | Since write operations are through adding elements to the end of the pinned vector via `push` and `extend`, two multi-threaded executions of a code that collects elements into the bag might result in the elements being collected in different orders. | Since write operations are through adding elements to the end of the pinned vector via `push` and `extend`, two multi-threaded executions of a code that collects elements into the bag might result in the elements being collected in different orders. | This is the main goal of this collection, allowing to collect elements concurrently and in the correct order. Although this does not seem trivial; it can be achieved almost trivially when `ConcurrentOrderedBag` is used together with a [`ConcurrentIter`](https://crates.io/crates/orx-concurrent-iter). |
| `into_inner` | Once the concurrent collection is completed, the bag can safely and cheaply be converted to its underlying `PinnedVec<T>`. | Once the concurrent collection is completed, the vec can safely be converted to its underlying `PinnedVec<ConcurrentOption<T>>`. Notice that elements are wrapped with a `ConcurrentOption` in order to provide thread safe concurrent read & write operations. | Growing through flexible setters allowing to write to any position, `ConcurrentOrderedBag` has the risk of containing gaps. `into_inner` call provides some useful metrics such as whether the number of elements pushed elements match the  maximum index of the vector; however, it cannot guarantee that the bag is gap-free. The caller is required to take responsibility to unwrap to get the underlying `PinnedVec<T>` through an `unsafe` call. |
|||||

## Benchmarks

### Performance with `push`

*You may find the details of the benchmarks at [benches/collect_with_push.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_push.rs).*

In the experiment, `rayon`s parallel iterator, `AppendOnlyVec`s and `ConcurrentVec`s `push` methods are used to collect results from multiple threads. Further, different underlying pinned vectors of the `ConcurrentVec` are tested. We observe that:
* The default `Doubling` growth strategy leads to efficient concurrent collection of results. Note that this variant does not require any input to construct.
* On the other hand, `Linear` growth strategy performs significantly better. Note that value of this argument means that each fragment of the underlying `SplitVec` will have a capacity of 2^12 (4096) elements. The underlying reason of improvement is potentially be due to less waste and could be preferred with minor knowledge of the data to be pushed.
* Finally, `Fixed` growth strategy is the least flexible and requires perfect knowledge about the hard-constrained capacity (will panic if we exceed). Since it does not outperform `Linear`, we do not necessarily prefer `Fixed` even if we have the perfect knowledge.

```bash
rayon/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [16.057 ms 16.390 ms 16.755 ms]
append_only_vec/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [23.679 ms 24.480 ms 25.439 ms]
concurrent_vec(Doubling)/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [14.055 ms 14.287 ms 14.526 ms]
concurrent_vec(Linear(12))/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [8.4686 ms 8.6396 ms 8.8373 ms]
concurrent_vec(Fixed)/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [9.8297 ms 9.9945 ms 10.151 ms]

rayon/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [43.118 ms 44.065 ms 45.143 ms]
append_only_vec/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [110.66 ms 114.09 ms 117.94 ms]
concurrent_vec(Doubling)/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [61.461 ms 62.547 ms 63.790 ms]
concurrent_vec(Linear(12))/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [37.420 ms 37.740 ms 38.060 ms]
concurrent_vec(Fixed)/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [43.017 ms 43.584 ms 44.160 ms]
```

The performance can further be improved by using `extend` method instead of `push`. You may see results in the next subsection and details in the [performance notes](https://docs.rs/orx-concurrent-bag/2.3.0/orx_concurrent_bag/#section-performance-notes) of `ConcurrentBag` which has similar characteristics.

### Performance with `extend`

*You may find the details of the benchmarks at [benches/collect_with_extend.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/collect_with_extend.rs).*

The only difference in this follow up experiment is that we use `extend` rather than `push` with `ConcurrentVec`. The expectation is that this approach will solve the performance degradation due to false sharing. Th expectation turns out to be true in this experiment and seems to have a significant impact:
* Extending rather than pushing might double the growth performance.
* There is not a significant difference between extending by batches of 64 elements or batches of 65536 elements. We do not need a well tuned number, a large enough batch size seems to be just fine.
* Not all scenarios allow to extend in batches; however, the significant performance improvement makes it preferable whenever possible.

```bash
rayon/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [16.102 ms 16.379 ms 16.669 ms]
append_only_vec/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [27.922 ms 28.611 ms 29.356 ms]
concurrent_vec(Doubling) | batch-size=64/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [8.7361 ms 8.8347 ms 8.9388 ms]
concurrent_vec(Linear(12)) | batch-size=64/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [4.2035 ms 4.2975 ms 4.4012 ms]
concurrent_vec(Fixed) | batch-size=64/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [4.9670 ms 5.0928 ms 5.2217 ms]
concurrent_vec(Doubling) | batch-size=16384/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [9.2441 ms 9.3988 ms 9.5594 ms]
concurrent_vec(Linear(12)) | batch-size=16384/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [3.5663 ms 3.6527 ms 3.7405 ms]
concurrent_vec(Fixed) | batch-size=16384/num_threads=8,num_items_per_thread-type=[16384]
                        time:   [5.0839 ms 5.2169 ms 5.3576 ms]

rayon/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [47.861 ms 48.836 ms 49.843 ms]
append_only_vec/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [125.52 ms 128.89 ms 132.41 ms]
concurrent_vec(Doubling) | batch-size=64/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [42.516 ms 43.097 ms 43.715 ms]
concurrent_vec(Linear(12)) | batch-size=64/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [20.025 ms 20.269 ms 20.521 ms]
concurrent_vec(Fixed) | batch-size=64/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [25.284 ms 25.818 ms 26.375 ms]
concurrent_vec(Doubling) | batch-size=65536/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [39.371 ms 39.887 ms 40.470 ms]
concurrent_vec(Linear(12)) | batch-size=65536/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [17.808 ms 17.923 ms 18.046 ms]
concurrent_vec(Fixed) | batch-size=65536/num_threads=8,num_items_per_thread-type=[65536]
                        time:   [24.291 ms 24.702 ms 25.133 ms]
```


## Contributing

Contributions are welcome! If you notice an error, have a question or think something could be improved, please open an [issue](https://github.com/orxfun/orx-concurrent-vec/issues/new) or create a PR.

## License

This library is licensed under MIT license. See LICENSE for details.
