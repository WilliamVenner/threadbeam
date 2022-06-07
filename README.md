[![crates.io](https://img.shields.io/crates/v/threadbeam.svg)](https://crates.io/crates/threadbeam)
[![docs.rs](https://docs.rs/threadbeam/badge.svg)](https://docs.rs/threadbeam/)
[![license](https://img.shields.io/crates/l/threadbeam)](https://github.com/WilliamVenner/threadbeam/blob/master/LICENSE)

# threadbeam

A simple, specialized channel type for beaming data out of a newly spawned thread.

# Usage

First, add `threadbeam` to your crate's dependencies in Cargo.toml:

```toml
[dependencies]
threadbeam = "0"
```

## Examples

```rust
let (tx, rx) = threadbeam::channel();

std::thread::spawn(move || {
    tx.send(String::from("Hello, world!"));
});

let hello = rx.recv();
assert_eq!(hello.as_deref(), Some("Hello, world!"));
```

```rust
let (hello, thread) = threadbeam::spawn(move |tx| {
    tx.send(String::from("Hello, world!"));
    // your code...
    String::from("Thread completed!")
});

assert_eq!(hello.as_deref(), Some("Hello, world!"));
assert_eq!(thread.join().ok().as_deref(), Some("Thread completed!"));
```

## [`parking_lot`](https://docs.rs/parking_lot/latest)

To use [`parking_lot`](https://docs.rs/parking_lot/latest) instead of the standard library's implementations of `Condvar` and `Mutex`, enable the `parking_lot` feature in your Cargo.toml:

```toml
[dependencies]
threadbeam = { version = "0", features = ["parking_lot"] }
```

## `no_std` via [`spin`](https://docs.rs/spin/latest)

For `no_std` environments, enable the `no_std` feature in your Cargo.toml:

This will use [`spin`](https://docs.rs/spin/latest) as the provider of the `Mutex` implementation.

```toml
[dependencies]
threadbeam = { version = "0", features = ["no_std"] }
```