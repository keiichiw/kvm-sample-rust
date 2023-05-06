# kvm-sample-rust

Minimal KVM API sample in Rust. This is inspired by [kvmsample](https://github.com/soulxu/kvmsample), which is written in C.

Abstraction layers such as safe structs are intentionally avoided in this project. Instead, it calls KVM API directly to make it easier to understand how each KVM ioctls should be called.
If you want safe wrappers of KVM APIs, other projects in [the section below](#learning-resources) would be helpful.

## Usage

```sh
# Create countdown.bin to run as a guest program
$ make
# Run countdown.bin as a KVM guest
$ cargo run -- ./countdown.bin
```

## Learning Resources

- [KVM API Documentation](https://docs.kernel.org/virt/kvm/api.html)
- [Using the KVM API - LWN](https://lwn.net/Articles/658511/)
- [rust-vmm/kvm_ioctls's example](https://docs.rs/kvm-ioctls/0.13.0/kvm_ioctls/#example---running-a-vm-on-x86_64)
- Existing VMMs written in Rust
  - [crosvm](https://crosvm.dev/book/)
  - [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor)
  - [firecracker](https://github.com/firecracker-microvm/firecracker)
