# Benchmarks

## Table of Contents

- [Overview](#overview)
- [Reproducing](#reproducing)
- [Benchmark Results](#benchmark-results)
    - [allocator-api](#allocator-api)
    - [warm-up](#warm-up)
    - [reset](#reset)
    - [vec](#vec)

## Overview

This directory contains two suites of benchmarks:

1. `allocator_api.rs`: `std::alloc::Allocator`-based benchmarks that aim to
   measure the performance of bump allocators within the generic `Allocator`
   API.

2. `benches.rs`: Miscellaneous Bumpalo-specific benchmarks.

The tables of benchmark results listed below are the results for the suite of
`std::alloc::Allocator`-based benchmarks. They are originally adapted from
[`blink-alloc`] (another fine bump allocator crate) which was already measuring
the relative performance between `blink-alloc` and `bumpalo`. I wasn't able to
reproduce many of their results showing that `blink-alloc` was faster than
`bumpalo`, however, which was part of the motivation to bring a subset of the
benchmarks into this repo and document reproduction steps.

Furthermore, the tables below include a `std::alloc::System` column, but their
results come with a few caveats. First, in order to implement a `reset` method
for the system allocator and deallocate everything that was allocated within a
certain region of code, I had to add additional bookkeeping to dynamically track
every live allocation. That bookkeeping generally won't be present in real
programs, which will instead use things like `Drop` implementations, so it makes
the system allocator's results look worse than they otherwise would
be. Additionally, these benchmarks are really designed to show off the strengths
of bump allocators and measure the operations that are important for bump
allocators. The system allocator is expected to perform worse, but that's
because it is designed for general purpose scenarios, where as bump allocators
are designed for very specific scenarios. These columns should mostly serve as
just a general reference point to get an idea of the magnitude of allocation
speed up you can expect in the very specific scenarios where using a bump
allocator makes sense.

Finally, all these benchmarks are synthetic. They are micro benchmarks. You
shouldn't expect that anything here will directly translate into speed ups for
your application. Application performance is what really matters, and things
observed in the micro often disappear in the macro. If your application isn't
bottlenecked on allocation, or can't abide by the constraints that a bump
allocator imposes, there's nothing that a bump allocator can do to improve its
performance.

[`blink-alloc`]: https://github.com/zakarumych/blink-alloc/blob/845b2db273371260eef2e9858386f6c6aa180e98/BENCHMARKS.md

## Reproducing

The `std::alloc::Allocator`-based benchmarks require using nightly Rust, since
the `Allocator` trait is still unstable. You must additionally enable Bumpalo's
`allocator_api` cargo feature:

```
$ cargo +nightly bench --bench allocator_api --features allocator_api
```

The miscellaneous benchmarks require Bumpalo's `collections` cargo feature:

```
$ cargo bench --bench benches --features collections
```

To update the tables below, use `cargo-criterion` and [`criterion-table`]:

```
$ cd bumpalo/benches/
$ cargo +nightly bench --features bench_allocator_api \
    --bench allocator_api \
    --message-format=json \
    > results.json
$ criterion-table < results.json > README.md
```

[`cargo-criterion`]: https://github.com/bheisler/cargo-criterion
[`criterion-table`]: https://github.com/nu11ptr/criterion-table

## Benchmark Results

### allocator-api

Benchmarks that measure calls into `std::alloc::Allocator` methods directly.

These operations are generally the ones that happen most often, and therefore
their performance is generally most important. Following the same logic, raw
allocation is generally the very most important.

|                                                     | `bumpalo::Bump`          | `blink_alloc::BlinkAlloc`       | `std::alloc::System`            |
| :-------------------------------------------------- | :----------------------- | :------------------------------ | :------------------------------ |
| **`allocate(u8) x 10007`**                          | `20.57 us` (✅ **1.00x**) | `17.59 us` (✅ **1.17x faster**) | `498.75 us` (❌ *24.24x slower*) |
| **`allocate(u32) x 10007`**                         | `19.12 us` (✅ **1.00x**) | `22.77 us` (❌ *1.19x slower*)   | `548.87 us` (❌ *28.71x slower*) |
| **`allocate(u64) x 10007`**                         | `19.13 us` (✅ **1.00x**) | `22.79 us` (❌ *1.19x slower*)   | `596.88 us` (❌ *31.20x slower*) |
| **`allocate(u128) x 10007`**                        | `20.33 us` (✅ **1.00x**) | `22.82 us` (❌ *1.12x slower*)   | `638.52 us` (❌ *31.41x slower*) |
| **`allocate([u8; 0]) x 10007`**                     | `20.85 us` (✅ **1.00x**) | `20.46 us` (✅ **1.02x faster**) | `222.39 us` (❌ *10.67x slower*) |
| **`allocate([u8; 1]) x 10007`**                     | `20.92 us` (✅ **1.00x**) | `20.39 us` (✅ **1.03x faster**) | `511.35 us` (❌ *24.44x slower*) |
| **`allocate([u8; 7]) x 10007`**                     | `20.79 us` (✅ **1.00x**) | `20.48 us` (✅ **1.02x faster**) | `607.00 us` (❌ *29.20x slower*) |
| **`allocate([u8; 8]) x 10007`**                     | `20.75 us` (✅ **1.00x**) | `20.45 us` (✅ **1.01x faster**) | `599.51 us` (❌ *28.89x slower*) |
| **`allocate([u8; 31]) x 10007`**                    | `20.81 us` (✅ **1.00x**) | `20.46 us` (✅ **1.02x faster**) | `702.04 us` (❌ *33.74x slower*) |
| **`allocate([u8; 32]) x 10007`**                    | `20.83 us` (✅ **1.00x**) | `20.44 us` (✅ **1.02x faster**) | `713.50 us` (❌ *34.25x slower*) |
| **`grow same align (u32 -> [u32; 2]) x 10007`**     | `48.41 us` (✅ **1.00x**) | `38.03 us` (✅ **1.27x faster**) | `1.18 ms` (❌ *24.30x slower*)   |
| **`grow smaller align (u32 -> [u16; 4]) x 10007`**  | `48.48 us` (✅ **1.00x**) | `38.14 us` (✅ **1.27x faster**) | `1.22 ms` (❌ *25.16x slower*)   |
| **`grow larger align (u32 -> u64) x 10007`**        | `39.88 us` (✅ **1.00x**) | `51.15 us` (❌ *1.28x slower*)   | `1.22 ms` (❌ *30.62x slower*)   |
| **`shrink same align ([u32; 2] -> u32) x 10007`**   | `20.21 us` (✅ **1.00x**) | `26.53 us` (❌ *1.31x slower*)   | `1.12 ms` (❌ *55.45x slower*)   |
| **`shrink smaller align (u32 -> u16) x 10007`**     | `20.28 us` (✅ **1.00x**) | `26.25 us` (❌ *1.29x slower*)   | `1.14 ms` (❌ *56.30x slower*)   |
| **`shrink larger align ([u16; 4] -> u32) x 10007`** | `21.25 us` (✅ **1.00x**) | `48.81 us` (❌ *2.30x slower*)   | `1.19 ms` (❌ *55.97x slower*)   |

### warm-up

Benchmarks that measure the first allocation in a fresh allocator.

These aren't generally very important, since the first allocation in a fresh
bump allocator only ever happens once by definition. This is mostly measuring
how long it takes the underlying system allocator to allocate the initial chunk
to bump allocate out of.

|                            | `bumpalo::Bump`          | `blink_alloc::BlinkAlloc`       | `std::alloc::System`          |
| :------------------------- | :----------------------- | :------------------------------ | :---------------------------- |
| **`first u32 allocation`** | `27.07 ns` (✅ **1.00x**) | `19.87 ns` (✅ **1.36x faster**) | `70.90 ns` (❌ *2.62x slower*) |

### reset

Benchmarks that measure the overhead of resetting a bump allocator to an empty
state, ready to be reused in a new program phase.

This generally doesn't happen as often as allocation, and therefore is generally
less important, but it is important to keep an eye on generally since
deallocation-en-masse and reusing already-allocated chunks can be selling points
for bump allocation over using a generic allocator in certain scenarios.

|                                         | `bumpalo::Bump`           | `blink_alloc::BlinkAlloc`        | `std::alloc::System`              |
| :-------------------------------------- | :------------------------ | :------------------------------- | :-------------------------------- |
| **`reset after allocate(u32) x 10007`** | `108.45 ns` (✅ **1.00x**) | `118.12 ns` (✅ **1.09x slower**) | `134.00 us` (❌ *1235.69x slower*) |

### vec

Benchmarks that measure the various `std::vec::Vec<T> operations when used in
conjuction with a bump allocator.

Bump allocators aren't often used directly, but instead through some sort of
collection. These benchmarks are important in the sense that the standard
`Vec<T>` type is probably the most-commonly used collection (although not
necessarily the most commonly used with bump allocators in Rust, at least until
the `Allocator` trait is stabilized).

|                                | `bumpalo::Bump`          | `blink_alloc::BlinkAlloc`        | `std::alloc::System`             |
| :----------------------------- | :----------------------- | :------------------------------- | :------------------------------- |
| **`push(usize) x 10007`**      | `13.77 us` (✅ **1.00x**) | `10.48 us` (✅ **1.31x faster**)  | `46.42 us` (❌ *3.37x slower*)    |
| **`reserve_exact(1) x 10007`** | `3.40 ms` (✅ **1.00x**)  | `42.70 us` (🚀 **79.56x faster**) | `702.93 us` (🚀 **4.83x faster**) |

---
Made with [criterion-table](https://github.com/nu11ptr/criterion-table)
