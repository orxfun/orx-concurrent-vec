# orx-concurrent-vec

An efficient, convenient and lightweight thread-safe grow-only read-and-write collection, ideal for collecting results concurrently.
* **convenient**: the vector can be shared among threads simply as a shared reference, not even requiring `Arc`,
* **efficient**: allows copy-free collecting which makes it performant especially when the type to be collected is not very small (please see the <a href="#section-benchmarks">benchmarks</a> section for tradeoffs and details),
* **lightweight**: a minimalistic implementation.

The vector preserves the order of elements with respect to the order the `push` method is called.

# Examples

Safety guarantees to push to the vector with an immutable reference makes it easy to share the vec among threads.

## Using `std::sync::Arc`

Following the common approach of using an `Arc`, we can share the vector among threads and collect results concurrently.

```rust
use orx_concurrent_vec::*;
use std::{sync::Arc, thread};

let (num_threads, num_items_per_thread) = (4, 8);

let convec = Arc::new(ConcurrentVec::new());
let mut thread_vec: Vec<thread::JoinHandle<()>> = Vec::new();

for i in 0..num_threads {
    let convec = convec.clone();
    thread_vec.push(thread::spawn(move || {
        for j in 0..num_items_per_thread {
            // concurrently collect results simply by calling `push`
            convec.push(i * 1000 + j);
        }
    }));
}

for handle in thread_vec {
    handle.join().unwrap();
}

let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
vec_from_convec.sort();
let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
expected.sort();
assert_eq!(vec_from_convec, expected);
```

## Using `std::thread::scope`

An even more convenient approach would be to use thread scopes. This allows to use shared reference of the vec across threads, instead of `Arc`.

```rust
use orx_concurrent_vec::*;
use std::thread;

let (num_threads, num_items_per_thread) = (4, 8);

let convec = ConcurrentVec::new();
let convec_ref = &convec; // just take a reference
std::thread::scope(|s| {
    for i in 0..num_threads {
        s.spawn(move || {
            for j in 0..num_items_per_thread {
                // concurrently collect results simply by calling `push`
                convec_ref.push(i * 1000 + j);
            }
        });
    }
});

let mut vec_from_convec: Vec<_> = convec.iter().copied().collect();
vec_from_convec.sort();
let mut expected: Vec<_> = (0..num_threads).flat_map(|i| (0..num_items_per_thread).map(move |j| i * 1000 + j)).collect();
expected.sort();
assert_eq!(vec_from_convec, expected);
```

# Safety

`ConcurrentVec` uses a [`SplitVec`](https://crates.io/crates/orx-split-vec) as the underlying storage.
`SplitVec` implements [`PinnedVec`](https://crates.io/crates/orx-pinned-vec) which guarantees that elements which are already pushed to the vector stay pinned to their memory locations.
This feature makes it safe to grow with a shared reference on a single thread, as implemented by [`ImpVec`](https://crates.io/crates/orx-imp-vec).

In order to achieve this feature in a concurrent program, `ConcurrentVec` pairs the `SplitVec` with an `AtomicUsize`.
* `AtomicUsize` fixes the target memory location of each element to be pushed at the time the `push` method is called. Regardless of whether or not writing to memory completes before another element is pushed, every pushed element receives a unique position reserved for it.
* `SplitVec` guarantees that already pushed elements are not moved around in memory and new elements are written to the reserved position.

The approach guarantees that
* only one thread can write to the memory location of an element being pushed to the vec,
* at any point in time, only one thread is responsible for the allocation of memory if the vec requires new memory,
* no thread reads an element which is being written, reading is allowed only after the element is completely written,
* hence, there exists no race condition.

This pair allows a lightweight and convenient concurrent vector which is ideal for collecting results concurrently.

# Write-Only vs Read-Write

The concurrent vec is read-and-write & grow-only vec which is convenient and efficient for collecting elements.
While allowing growth by pushing elements from multiple threads, each thread can safely read already pushed elements.

See [`ConcurrentBag`](https://crates.io/crates/orx-concurrent-bag) for a write-only variant which allows only writing during growth.
The advantage of the bag, on the other hand, is that it stores elements as `T` rather than `Option<T>`.

<div id="section-benchmarks"></div>

# Benchmarks

*You may see the benchmark at [benches/grow.rs](https://github.com/orxfun/orx-concurrent-vec/blob/main/benches/grow.rs).*

In this benchmark, concurrent results are collected using `ConcurrentVec` together with scoped threads and `Arc`. Computation time performance of these two is negligible, hence, only scoped thread implementation is reported. Results are compared by the `collect` method `rayon`s parallel iterator. 

<img src="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_grow.PNG" alt="https://raw.githubusercontent.com/orxfun/orx-concurrent-vec/main/docs/img/bench_grow.PNG" />

We can see that:
* `rayon` is extremely performant when the data size to be collected is small and there is a huge concurrency load. We can see that it outperforms `ConcurrentVec` when the threads do not do any work at all to produce outputs and the output data is `i32`.
* On the other hand, when there exists some work to be done to produce the outputs (workload), `ConcurrentVec` starts to perform significantly faster.
* Similarly, when the output data is large (`[i32; 32]` in this example), regardless of the additional workload, `ConcurrentVec` performs faster.
