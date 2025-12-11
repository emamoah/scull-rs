# Scull - Simple Character Utility for Loading Localities

This project is an attempt to reimplement the linux device driver samples from the book "[Linux Device Drivers, 3rd Edition](https://lwn.net/Kernel/LDD3/)" (LDD3), originally written in C, in the Rust language.

[Rust for Linux](https://rust-for-linux.com/) is an ongoing project to integrate Rust into the linux kernel. Abstractions are being developed to enable drivers written in Rust interface easily with the existing API's.

The code in this project is currently not on par with the reference sample, and should be updated as the relevant abstractions are available.

## Building

The repo consists of out-of-tree kernel modules which need to be built against a rust-enabled kernel (`CONFIG_RUST=y`). For more information, visit the [kernel docs](https://docs.kernel.org/rust/quick-start.html).

**Minimum kernel version: 6.18.0**

### Examples

The following commands assume you have a full kernel source tree with rust configured.

Building the modules:

```shell
$ make LLVM=1 KDIR=/path/to/kernel
```

To enable [rust-analyzer](https://rust-analyzer.github.io/) for IDE support:

```shell
$ make LLVM=1 KDIR=/path/to/kernel rust-analyzer
```
