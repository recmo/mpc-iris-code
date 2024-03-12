# MPC Iris Code

**DO NOT USE IN PROD**

Experiments to see if iris codes can be matched in MPC with acceptable privacy and performance.

## Install

Make sure to optimize for the correct target CPU to make use of SVE features. To do this set the compiler flag to optimize for the target CPU:

```sh
RUSTFLAGS="-Ctarget-cpu=native" cargo install --git https://github.com/recmo/mpc-iris-code
```

When cross-compiling as source checkout from a different environment to Graviton 3, set the cpu explicitly:

```sh
RUSTFLAGS="-Ctarget-cpu=neoverse-v1" cargo build --release --target aarch64-unknown-linux-gnu
```

To explore assembly output in [Godbolt], use the following compiler options:

[Godbolt]: https://rust.godbolt.org/

```
--edition=2021 --target aarch64-unknown-linux-gnu -C opt-level=3 -C lto=fat --C target-cpu=neoverse-v1
```

Some useful resources for low-level Apple Silicon and Graviton optimization:

* https://dougallj.github.io/applecpu/firestorm.html
* https://chipsandcheese.com/2022/05/29/graviton-3-first-impressions/


## Specification

See the [Specification](specification.ipynb) notebook. (Moved to a notebook because of [this issue](https://github.com/github/markup/issues/1551)).


## Benchmarking

Using [samply]:

[samply]: https://github.com/mstange/samply

```sh
RUSTFLAGS="-Ctarget-cpu=native" samply record --rate 10000 cargo bench --profile profiling --bench bench --features bench
```
