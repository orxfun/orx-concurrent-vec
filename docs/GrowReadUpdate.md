# Safe Concurrent Grow & Read & Update

`ConcurrentVec` provides safe api for the following three sets of concurrent operations: **growth** & **read** & **update**.

## Growth

ConcurrentVec is designed with having high performance growth in focus.
* `push` allows to push elements one at a time, returning the index in the vector that the value is written to.
* `extend` allows pushing all values that an iterator yields next to each other, returning the index in the vector that the first value is written to. Note that `extend` is a great tool to deal with "false sharing" and might provide significant performance improvements when high-performance growth is required (see the [benchmarks](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/#benchmarks)).

## Read

Safe api of `ConcurrentVec` and `ConcurrentSlice` does not provide out `&T` or `&mut T` references to guarantee a safe access to the elements. Instead, it provides a reference to a concurrent element: `vec.get[i]` and `vec[i]` returns `Option<&ConcurrentElement<T>>` and `&ConcurrentElement<T>`, respectively.

[`ConcurrentElement`](https://docs.rs/orx-concurrent-vec/latest/orx_concurrent_vec/struct.ConcurrentElement.html) provides thread-safe means to work with the data, such as:
* `map(f)` where `f: FnOnce(&T) -> U`, returns the result of the map operation. Of course, `U` can be the unit type when we only need to access the data but not a result.
* `cloned()` returns a clone of value of the element.

Alternatively, when we only need the clone of the value, we can directly get it from the vector via `vec.get_cloned(i)`.

## Update

We can also concurrently update the values of elements. Similarly, mutation is not through providing `&mut T` but instead using the thread-safe methods of `ConcurrentElement`:
* `update(f)` where `f: FnMut(&mut T)` applies the update function on the value; this can be considered as the thread-safe counter-part of `get_mut`.
* `set(value)` overwrites the element's value.
* `replace(value)` is similar to *set* except that it additionally returns the prior value.

## Example - #1

We can execute all these three sets of operations concurrently from multiple threads in the absence of race conditions and locks.

This is demonstrated in the [random_actions](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/random_actions.rs) example. Here, we spawn a number of threads, each of which continuously randomly picks an action and applies it concurrently. It is a toy example, however, demonstrates possible thread-safe concurrent actions and how to use them. 

```
cargo run --example random_actions -- --help
```

```rust ignore
fn apply_random_concurrent_operations(
    vec: &ConcurrentVec<String>,
    final_vec_len: usize,
    mut r: ChaCha8Rng,
) {
    while vec.len() < final_vec_len {
        match ConAction::new(&mut r, vec.len()) {
            ConAction::Push(value) => {
                vec.push(value);
            }
            ConAction::Extend(iter) => {
                vec.extend(iter);
            }
            ConAction::Map(i) => {
                // when we know i is in-bounds
                let _num_chars = vec[i].map(|x: &String| x.len());
            }
            ConAction::Clone(i) => {
                let _clone: Option<String> = vec.get_cloned(i);
            }
            ConAction::Replace(i, new_value) => {
                // when we are not sure if i is in or out of bounds
                if let Some(elem) = vec.get(i) {
                    let old_value = elem.replace(new_value);
                    assert!(old_value.parse::<usize>().is_ok());
                }
            }
            ConAction::Set(i, new_value) => {
                // when we know i is in-bounds
                vec[i].set(new_value);
            }
            ConAction::Update(i, c) => {
                if let Some(elem) = vec.get(i) {
                    elem.update(|x| x.push(c));
                }
            }
            ConAction::IterMapReduce => {
                let sum: usize = vec
                    .iter()
                    .map(|elem| elem.map(|x| x.parse::<usize>().unwrap()))
                    .sum();
                assert!(sum > 0);
            }
            ConAction::IterCloned(range) => {
                let _collected: Vec<_> = vec.slice(range).iter_cloned().collect();
            }
            ConAction::IterMutate => {
                vec.iter().for_each(|elem| {
                    elem.update(|x: &mut String| {
                        if x.len() > 1 {
                            x.pop();
                        }
                    });
                });
            }
        }
    }
}
```

## Example - #2

In the second example, each thread have their own responsibilities: some of them add elements to the vector, some update existing elements and some read data. This is demonstrated in the [concurrent_grow_read_update](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/concurrent_grow_read_update.rs) example.

```rust ignore
use orx_concurrent_vec::*;

let con_vec = ConcurrentVec::new();

std::thread::scope(|s| {
    for _ in 0..args.num_pushers {
        s.spawn(|| push(&con_vec, num_items_per_thread, args.lag));
    }

    for _ in 0..args.num_extenders {
        s.spawn(|| extend(&con_vec, num_items_per_thread, 64, args.lag));
    }

    for _ in 0..args.num_readers {
        s.spawn(|| read(&con_vec, final_len, args.lag));
    }

    for _ in 0..args.num_updaters {
        s.spawn(|| update(&con_vec, final_len, args.lag));
    }

    for _ in 0..args.num_iterators {
        s.spawn(|| iterate(&con_vec, final_len));
    }
});

// convert into "non-concurrent" vec
let vec = con_vec.into_inner();
for (i, x) in vec.iter().enumerate() {
    assert_eq!(x, &i.to_string());
}
```

You may find the complete example here: [concurrent_grow_read_update](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/concurrent_grow_read_update.rs) or try out:

```
cargo run --example concurrent_grow_read_update -- --help
```

