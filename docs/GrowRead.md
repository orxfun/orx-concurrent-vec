# Safe Concurrent Grow & Read with Unsafe Api

`ConcurrentVec` provides safe api for the following three sets of concurrent operations: **growth** & **read** & **update**. Please see [GrowReadUpdate](https://github.com/orxfun/orx-concurrent-vec/blob/main/docs/GrowReadUpdate.md) for details.

Further, it provides unsafe api for direct shared and mutable access to the elements; i.e., `&T` and `&mut T`, with the following methods:

* `get_ref(i)` and `get_mut(i)` return `Option<&T>` and `Option<&mut T>`, which will be None if *i* is out of bounds.
* `iter_ref()` and `iter_mut()` return iterators yielding `&T` and `&mut T`, respectively.

As it will be explained below that these references will always be valid. However, ConcurrentVec also allows concurrent mutation of existing elements. Therefore, once the references are leaked out, it does not have a means to prevent race conditions. Therefore, these methods are marked as unsafe.

The access is partially safe and partially unsafe.

## Safety Guarantees

Reference obtained by these method will be valid and will remain valid:
* `ConcurrentVec` prevents access to elements which are not yet completely initialized. Therefore, it prevents data race during initialization.
* Underlying `PinnedVec` storage makes sure that memory location of the elements never change.
Therefore, the caller can hold on to the obtained references throughout the lifetime of the vec.
It is guaranteed that the references will be valid and pointing to the correct elements.

## Unsafe Bits

There might be read & update accesses to the elements through the safe api of the ConcurrentVec. This will cause data races in the following conditions:
* When we are reading the element through `&T` obtained by an unsafe method, another thread uses ConcurrentVec's thread-safe methods such as *update*, *set* or *replace* to mutate the data. This leads to a data race and an undefined behavior.
* When we are updating the value of the element through `&mut T` obtained by an unsafe method, another thread tries to read the value by thread-safe access methods such as *map* or *get_cloned*. This again leads to a data race and an undefined behavior.

## Safe Usage

If we can guarantee that the two scenarios described above do not happen concurrently, we can use the unsafe api. This requires safe wrappers.

One common use case, however, occurs when we do not need to update values of the elements. Then, it is clear that the first scenario becomes safe. We can define this scenario as follows:
* We can push to or extend the vector concurrently. Recall that we do not need to worry about growth methods since ConcurrentVec will not allow unsafe access to not-initialized elements or change the memory locations of already added elements.
* We can concurrently read already added elements. We can do this through the `ConcurrentElement`'s thread safe methods. Alternatively, we can use the unsafe methods `get_ref` or `iter_ref` to directly access to the elements. Further, we can hold on to these references throughout the lifetime of the concurrent vec.
* However, we cannot use:
  * the thread-safe update methods of `ConcurrentElement` such as *set*, *replace* or *update*, and
  * unsafe methods providing a mutable references, i.e., `get_mut` and `iter_mut`.

Then, this program will be data race free and safe.

## Example

To summarize, we can execute
* thread safe growth & read methods, and
* unsafe read methods

concurrently from multiple threads in the absence of race conditions and locks. 

This is demonstrated in the [concurrent_grow_read_noupdate](https://github.com/orxfun/orx-concurrent-vec/blob/main/examples/concurrent_grow_read_noupdate.rs) example.

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
cargo run --example concurrent_grow_read_noupdate -- --help
```

