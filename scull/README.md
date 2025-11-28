# Scull

A driver module for a virtual character device, implemented as a singly-linked list of blocks, each consisting of a two-dimensional array of bytes.

This module is currently implemented as a *Miscdevice* driver, rather than a char device driver as in the original code, and creates a single device file at `/dev/scull`.

## Current limitations

- The device is not persistent. Each `open()` returns a fresh state.
- The device is not seekable, so reading previously written data is not possible.
- Due to the absence of global state, the `ioctl` implementation only supports two read operations: `SCULL_IOCGQUANTUM` and `SCULL_IOCGQSET`, which return the immutable configuration of the *currently opened* device, rather than a mutable global value.
