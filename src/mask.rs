use orx_pinned_vec::ConcurrentPinnedVec;
use orx_split_vec::{ConcurrentSplitVec, Doubling, PinnedVec, SplitVec};
use std::{
    fmt::Debug,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub struct Mask {
    is_growing: AtomicBool,
    safe_cap: AtomicUsize,
    available: ConcurrentSplitVec<AtomicBool, Doubling>,
}

impl Debug for Mask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mask")
            .field("is_growing", &self.is_growing)
            .finish()
    }
}

const ORD: Ordering = Ordering::SeqCst;

impl Mask {
    pub fn new(initial_len: usize) -> Self {
        let mut data = SplitVec::with_doubling_growth_and_fragments_capacity(32);
        for _ in 0..initial_len {
            data.push(AtomicBool::new(true));
        }
        let initial_capacity = data.capacity();
        for _ in initial_len..initial_capacity {
            data.push(AtomicBool::new(false));
        }
        let data = data.into();
        let safe_len = initial_capacity.into();

        Self {
            is_growing: AtomicBool::new(false),
            available: data,
            safe_cap: safe_len,
        }
    }

    #[inline(always)]
    pub fn is_growing(&self) -> bool {
        self.is_growing.load(ORD)
    }

    pub fn len(&self) -> usize {
        let safe_cap = self.safe_cap.load(ORD);
        self.available.capacity().min(safe_cap)
    }

    pub fn taken_until(&self, end_idx_exclusive: usize) {
        while self.is_growing() {}

        if self.available.capacity() < end_idx_exclusive {
            self.grow_to(end_idx_exclusive);
        }
    }

    pub fn written(&self, begin_idx: usize, end_idx_exclusive: usize) {
        while self.is_growing() {}

        for i in begin_idx..end_idx_exclusive {
            let p = unsafe { self.available.get(i) }.expect("exists");
            p.store(true, ORD);
        }
    }

    pub fn is_available(&self, index: usize) -> bool {
        match unsafe { self.available.get(index) } {
            Some(p) => p.load(ORD),
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.is_growing.store(true, ORD);

        let capacity = self.available.capacity();
        for i in 0..capacity {
            let p = unsafe { self.available.get_ptr_mut(i) };
            unsafe { p.write(AtomicBool::new(false)) };
        }

        self.is_growing.store(false, ORD);
    }

    pub fn num_available(&self, len: usize) -> usize {
        let safe_len = self.safe_cap.load(ORD);
        let len = len.min(self.available.capacity()).min(safe_len);
        let mut count = 0;
        for i in 0..len {
            let e = unsafe { self.available.get(i) }.expect("exists");
            if e.load(ORD) {
                count += 1;
            }
        }

        count
    }

    // locked
    fn grow_to(&self, new_capacity: usize) {
        while self
            .is_growing
            .compare_exchange_weak(false, true, ORD, ORD)
            .is_err()
        {}

        let initial_capacity = self.available.capacity();
        let new_capacity = self
            .available
            .grow_to(new_capacity)
            .expect("cannot grow further");

        for i in initial_capacity..new_capacity {
            let p = unsafe { self.available.get_ptr_mut(i) };
            unsafe { p.write(AtomicBool::new(false)) };
        }

        self.safe_cap.store(new_capacity, ORD);

        self.is_growing.store(false, ORD);
    }
}
