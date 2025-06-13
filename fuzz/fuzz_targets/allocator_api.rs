#![no_main]
use std::{alloc::Layout, ops::Range, ptr::NonNull};

use allocator_api2::alloc::Allocator;
use arbitrary::{Arbitrary, Unstructured};
use bumpalo::Bump;
use core::fmt::Debug;
use log::debug;
use rangemap::RangeSet;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|fuzz: Fuzz| fuzz.run());

// Like `std::dbg` but using `log::debug!` instead of `eprintln!`.
macro_rules! debug_dbg {
    // NOTE: We cannot use `concat!` to make a static string as a format argument
    // of `eprintln!` because `file!` could contain a `{` or
    // `$val` expression could be a block (`{ .. }`), in which case the `eprintln!`
    // will be malformed.
    () => {
        ::log::debug!("[{}:{}:{}]", file!(), line!(), column!())
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                ::log::debug!("[{}:{}:{}] {} = {:#?}",
                    file!(), line!(), column!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

#[derive(Debug, Arbitrary)]
pub struct Fuzz {
    min_align: MinAlign,
    operations: Vec<Operation>,
}

impl Fuzz {
    pub fn run(self) {
        match self.min_align {
            MinAlign::Shl0 => self.run_align::<1>(),
            MinAlign::Shl1 => self.run_align::<2>(),
            MinAlign::Shl2 => self.run_align::<4>(),
            MinAlign::Shl3 => self.run_align::<8>(),
            MinAlign::Shl4 => self.run_align::<16>(),
        }
    }

    fn run_align<const MIN_ALIGN: usize>(self) {
        debug_dbg!(MIN_ALIGN);

        let bump: Bump<MIN_ALIGN> = Bump::with_min_align_and_capacity(32);

        let mut allocations = vec![];
        let mut used_ranges = UsedRanges::default();

        #[allow(clippy::unused_enumerate_index)]
        for (_operation_i, &operation) in self.operations.iter().enumerate() {
            debug!("======================================");
            debug!("OPERATION {_operation_i}");
            debug_dbg!(&allocations);
            debug_dbg!(&used_ranges);
            debug_dbg!(&bump);

            match operation {
                Operation::Allocate { layout, zero } => unsafe {
                    let layout = layout.0;

                    let ptr = if zero {
                        (&bump).allocate_zeroed(layout)
                    } else {
                        (&bump).allocate(layout)
                    };

                    debug!("ALLOCATE");
                    debug_dbg!(layout);
                    debug_dbg!(&ptr);

                    if let Ok(ptr) = ptr {
                        assert_eq!(ptr.len(), layout.size());
                        assert!(is_aligned_to(ptr, layout.align()));
                        used_ranges.insert(ptr);

                        if zero {
                            assert_zeroed(ptr);
                        }

                        initialize(ptr);
                        assert_initialized(ptr);

                        allocations.push(Allocation { ptr, layout });
                    }
                },
                Operation::Deallocate { index } => unsafe {
                    if allocations.is_empty() {
                        continue;
                    }

                    let i = index % allocations.len();
                    let Allocation { ptr, layout } = allocations.swap_remove(i);

                    debug!("DEALLOCATE");
                    debug_dbg!(layout);
                    debug_dbg!(&ptr);

                    assert_eq!(ptr.len(), layout.size());
                    assert!(is_aligned_to(ptr, layout.align()));
                    used_ranges.remove(ptr);

                    assert_initialized(ptr);
                    deinitialize(ptr);

                    (&bump).deallocate(ptr.cast(), layout);
                },
                Operation::Reallocate {
                    index,
                    layout: new_layout,
                    zero,
                } => unsafe {
                    let mut new_layout = new_layout.0;

                    debug!("REALLOCATE");
                    debug_dbg!(new_layout);
                    debug_dbg!(index);

                    if allocations.is_empty() {
                        debug!("CANCELLED: NO ALLOCATIONS");
                        continue;
                    }

                    let i = index % allocations.len();
                    debug_dbg!(i);

                    let Allocation {
                        ptr: old_ptr,
                        layout: old_layout,
                    } = allocations[i];

                    debug_dbg!(old_layout);
                    debug_dbg!(old_ptr);

                    assert_eq!(old_ptr.len(), old_layout.size());
                    assert!(is_aligned_to(old_ptr, old_layout.align()));
                    assert_initialized(old_ptr);

                    let new_ptr = if new_layout.size() > old_layout.size() {
                        if zero {
                            (&bump).grow_zeroed(old_ptr.cast(), old_layout, new_layout)
                        } else {
                            (&bump).grow(old_ptr.cast(), old_layout, new_layout)
                        }
                    } else {
                        (&bump).shrink(old_ptr.cast(), old_layout, new_layout)
                    };

                    debug_dbg!(&new_ptr);

                    if let Ok(new_ptr) = new_ptr {
                        #[allow(ambiguous_wide_pointer_comparisons)]
                        if new_ptr == old_ptr {
                            assert_eq!(new_ptr.len(), old_layout.size());
                            new_layout =
                                Layout::from_size_align(old_layout.size(), new_layout.align())
                                    .unwrap();
                        } else {
                            assert_eq!(new_ptr.len(), new_layout.size());
                        }

                        assert!(is_aligned_to(new_ptr, new_layout.align()));

                        if new_layout.size() > old_layout.size() {
                            let [old_part, new_part] = split_slice(new_ptr, old_layout.size());
                            assert_initialized(old_part);

                            if zero {
                                assert_zeroed(new_part);
                            }

                            initialize(new_ptr);
                        }

                        assert_initialized(new_ptr);

                        used_ranges.remove(old_ptr);
                        used_ranges.insert(new_ptr);

                        allocations[i] = Allocation {
                            ptr: new_ptr,
                            layout: new_layout,
                        }
                    }
                },
            }
        }

        debug!("====================================");
        debug!("DONE WITH ALL OPERATIONS");
        debug!("DROPPING REMAINING ALLOCATIONS");
        debug!("====================================");

        unsafe {
            for Allocation { ptr, layout } in allocations {
                debug_dbg!(layout);
                debug_dbg!(ptr);
                used_ranges.remove(ptr);
                assert_initialized(ptr);
                deinitialize(ptr);
                (&bump).deallocate(ptr.cast(), layout);
            }
        }
    }
}

#[derive(Default)]
struct UsedRanges {
    used: RangeSet<usize>,
}

impl Debug for UsedRanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_list();

        for range in self.used.iter() {
            list.entry(&HexRange(range));
        }

        list.finish()
    }
}

impl UsedRanges {
    /// Marks a pointer range as used.
    ///
    /// # Panics
    ///
    /// Panics if this pointer range overlaps with with any used range.
    fn insert(&mut self, ptr: NonNull<[u8]>) {
        let range = addr_range(ptr);

        if !range.is_empty() {
            assert!(
                !self.used.overlaps(&range),
                "insert failed: range={:?} used={:?}",
                HexRange(&range),
                self.used
            );
            self.used.insert(range);
        }
    }

    /// Marks a pointer range as unused.
    ///
    /// # Panics
    ///
    /// Panics if this pointer range overlaps with any unused range.
    fn remove(&mut self, ptr: NonNull<[u8]>) {
        let range = addr_range(ptr);

        if !range.is_empty() {
            assert_eq!(
                self.used.gaps(&range).map(|r| r.len()).sum::<usize>(),
                0,
                "remove failed: range={:?} used={:?}",
                HexRange(&range),
                self.used
            );
            self.used.remove(range);
        }
    }
}

/// Wrapper for a prettier `Debug` impl for `Range<usize>` in the context of pointer ranges.
struct HexRange<'a>(&'a Range<usize>);

impl Debug for HexRange<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Range { start, end } = self.0.clone();
        let len = end - start;
        write!(f, "{start:x}..{end:x} ({len})")
    }
}

fn split_slice(ptr: NonNull<[u8]>, mid: usize) -> [NonNull<[u8]>; 2] {
    assert!(mid <= ptr.len());

    let lhs_len = mid;
    let rhs_len = ptr.len() - mid;

    let lhs_ptr = ptr.cast::<u8>();
    let rhs_ptr = unsafe { ptr.cast::<u8>().add(mid) };

    [
        NonNull::slice_from_raw_parts(lhs_ptr, lhs_len),
        NonNull::slice_from_raw_parts(rhs_ptr, rhs_len),
    ]
}

fn addr_range(ptr: NonNull<[u8]>) -> Range<usize> {
    let addr = ptr.as_ptr().addr();
    addr..addr + ptr.len()
}

/// Writes a pattern that can later be asserted to still be the same using [`assert_initialized`].
unsafe fn initialize(ptr: NonNull<[u8]>) {
    for i in 0..ptr.len() {
        ptr.cast::<u8>().as_ptr().add(i).write(i as u8);
    }
}

/// Asserts that the bytes still have the same pattern as when it was set using [`initialize`].
unsafe fn assert_initialized(ptr: NonNull<[u8]>) {
    for i in 0..ptr.len() {
        assert_eq!(ptr.cast::<u8>().as_ptr().add(i).read(), i as u8);
    }
}

// Writes a new pattern to the bytes that can't be mistaken for initialized or zeroed bytes.
unsafe fn deinitialize(ptr: NonNull<[u8]>) {
    ptr.as_ptr().cast::<u8>().write_bytes(0xFA, ptr.len())
}

// Asserts that all bytes are zero.
unsafe fn assert_zeroed(ptr: NonNull<[u8]>) {
    ptr.as_ptr().cast::<u8>().write_bytes(0, ptr.len())
}

#[derive(Debug)]
struct Allocation {
    ptr: NonNull<[u8]>,
    layout: Layout,
}

#[derive(Debug, Clone, Copy, Arbitrary)]
enum Operation {
    Allocate {
        layout: FuzzLayout,
        zero: bool,
    },
    Deallocate {
        index: usize,
    },
    Reallocate {
        index: usize,
        layout: FuzzLayout,
        zero: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct FuzzLayout(Layout);

impl<'a> Arbitrary<'a> for FuzzLayout {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let size = u.int_in_range(0..=512)?;
        let align = 1 << u.int_in_range(1..=8)?;
        Ok(FuzzLayout(Layout::from_size_align(size, align).unwrap()))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <[usize; 2]>::size_hint(depth)
    }
}

#[derive(Debug, Clone, Copy, Arbitrary)]
enum MinAlign {
    Shl0 = 1 << 0,
    Shl1 = 1 << 1,
    Shl2 = 1 << 2,
    Shl3 = 1 << 3,
    Shl4 = 1 << 4,
}

fn is_aligned_to<T: ?Sized>(ptr: NonNull<T>, align: usize) -> bool {
    if !align.is_power_of_two() {
        panic!("is_aligned_to: align is not a power-of-two");
    }

    ptr.addr().get() & (align - 1) == 0
}
