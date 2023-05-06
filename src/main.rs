use std::ffi::c_uint;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::ptr::null_mut;

use kvm_bindings::*;
use vmm_sys_util::errno;
use vmm_sys_util::ioctl::{ioctl, ioctl_with_mut_ref, ioctl_with_ref, ioctl_with_val};
use vmm_sys_util::{ioctl_io_nr, ioctl_ioc_nr, ioctl_ior_nr, ioctl_iow_nr};

// Define KVM ioctls
const KVMIO: c_uint = 0xAE;
ioctl_io_nr!(KVM_GET_API_VERSION, KVMIO, 0x00);
ioctl_io_nr!(KVM_CREATE_VM, KVMIO, 0x01);
ioctl_iow_nr!(
    KVM_SET_USER_MEMORY_REGION,
    KVMIO,
    0x46,
    kvm_userspace_memory_region
);
ioctl_io_nr!(KVM_CREATE_VCPU, KVMIO, 0x41);
ioctl_io_nr!(KVM_GET_VCPU_MMAP_SIZE, KVMIO, 0x04);
ioctl_io_nr!(KVM_RUN, KVMIO, 0x80);
ioctl_iow_nr!(KVM_SET_REGS, KVMIO, 0x82, kvm_regs);
ioctl_ior_nr!(KVM_GET_SREGS, KVMIO, 0x83, kvm_sregs);
ioctl_iow_nr!(KVM_SET_SREGS, KVMIO, 0x84, kvm_sregs);

// Read a binary file into a Vec<u8>
fn read_binary(path: &str) -> Vec<u8> {
    let mut asm_code = Vec::new();
    let mut buf = BufReader::new(File::open(path).unwrap());
    buf.read_to_end(&mut asm_code).unwrap();
    asm_code
}

fn main() {
    let mut args = std::env::args();
    if args.len() != 2 {
        eprintln!("Usage: {} <binary file>", args.nth(0).unwrap());
        return;
    }
    let binary = args.nth(1).unwrap();
    let asm_code = read_binary(&binary);

    // Open /dev/kvm
    let dev = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .unwrap();

    // Get KVM API version
    let kvm_api_version = unsafe { ioctl(&dev, KVM_GET_API_VERSION()) };
    println!("KVM API version: {}", kvm_api_version);

    // Create VM
    let ret = unsafe { ioctl(&dev, KVM_CREATE_VM()) };
    if ret < 0 {
        panic!("KVM_CREATE_VM failed: {}", errno::Error::last());
    }
    let vm_fd: File = unsafe { File::from_raw_fd(ret) };

    // Set memory
    let mem_size = 1024000;
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };
    if load_addr == libc::MAP_FAILED as *mut u8 {
        panic!("mmap failed");
    }
    let ret = unsafe {
        ioctl_with_ref(
            &vm_fd,
            KVM_SET_USER_MEMORY_REGION(),
            &kvm_userspace_memory_region {
                slot: 0,
                guest_phys_addr: 0,
                memory_size: mem_size as u64,
                userspace_addr: load_addr as u64,
                flags: 0,
            },
        )
    };
    if ret < 0 {
        panic!(
            "KVM_SET_USER_MEMORY_REGION failed: {}",
            errno::Error::last()
        );
    }

    // Load assmebly code to memory
    unsafe {
        let mut slice = std::slice::from_raw_parts_mut(load_addr, mem_size);
        slice.write_all(&asm_code).unwrap();
    }

    // Initialize vCPU
    let vcpu_id = 0;
    let vcpu_fd = unsafe { ioctl_with_val(&vm_fd, KVM_CREATE_VCPU(), vcpu_id) };
    if vcpu_fd < 0 {
        panic!("KVM_CREATE_VCPU failed: {}", errno::Error::last());
    }
    let vcpu_fd: File = unsafe { File::from_raw_fd(vcpu_fd) };
    let kvm_run_mmap_size = unsafe { ioctl(&dev, KVM_GET_VCPU_MMAP_SIZE()) };
    if kvm_run_mmap_size < 0 {
        panic!("KVM_GET_VCPU_MMAP_SIZE failed: {}", errno::Error::last());
    }
    let kvm_run_ptr = unsafe {
        libc::mmap(
            null_mut(),
            kvm_run_mmap_size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            vcpu_fd.as_raw_fd(),
            0,
        )
    };
    if kvm_run_ptr == libc::MAP_FAILED {
        panic!("mmap failed");
    }

    // Reset sregs
    let mut sregs: kvm_sregs = unsafe { std::mem::zeroed() };
    let ret = unsafe { ioctl_with_mut_ref(&vcpu_fd, KVM_GET_SREGS(), &mut sregs) };
    if ret < 0 {
        panic!("KVM_GET_SREGS failed: {}", errno::Error::last());
    }
    let selector = 0;
    let base = 0;
    sregs.cs.selector = selector;
    sregs.cs.base = base;

    let ret = unsafe { ioctl_with_ref(&vcpu_fd, KVM_SET_SREGS(), &sregs) };
    if ret < 0 {
        panic!("KVM_SET_SREGS failed: {}", errno::Error::last());
    }

    // Reset REGS
    let mut regs: kvm_regs = unsafe { std::mem::zeroed() };
    regs.rflags = 0x2u64;
    regs.rip = 0x0;
    regs.rsp = 0xffffffff;
    regs.rbp = 0;
    let ret = unsafe { ioctl_with_mut_ref(&vcpu_fd, KVM_SET_REGS(), &mut regs) };
    if ret < 0 {
        panic!("KVM_SET_REGS failed: {}", errno::Error::last());
    }

    // Run VM
    loop {
        // KVM_RUN blocks until the VM exits
        let ret = unsafe { ioctl(&vcpu_fd, KVM_RUN()) };
        if ret < 0 {
            panic!("KVM_RUN failed: {}", errno::Error::last());
        }
        let kvm_run = kvm_run_ptr as *mut kvm_run;
        let exit_reason = unsafe { (*kvm_run).exit_reason };
        // Check the exit reason and handle it appropriately
        match exit_reason {
            KVM_EXIT_HLT => {
                println!("KVM_EXIT_HLT");
                break;
            }
            KVM_EXIT_IO => {
                let direction = unsafe { (*kvm_run).__bindgen_anon_1.io.direction };
                let offset = unsafe { (*kvm_run).__bindgen_anon_1.io.data_offset } as usize;
                if direction == KVM_EXIT_IO_OUT as u8 {
                    println!("KVM_EXIT_IO_OUT");
                    // TODO: Support `io.size`
                    println!(
                        "port[{}]: {}",
                        unsafe { (*kvm_run).__bindgen_anon_1.io.port },
                        unsafe {
                            *(((kvm_run_ptr as *const u8).offset(offset as isize)) as *mut i32)
                        }
                    );
                } else {
                    println!("KVM_EXIT_IO_IN");
                    let num;
                    loop {
                        let mut s = String::new();
                        print!("input an integer> ");
                        std::io::stdout().flush().unwrap();
                        std::io::stdin()
                            .read_line(&mut s)
                            .expect("Did not enter a correct string");
                        if let Ok(n) = s.trim().parse::<i32>() {
                            num = n;
                            break;
                        } else {
                            eprintln!("Invalid input");
                        }
                    }
                    unsafe {
                        *(((kvm_run_ptr as *const u8).offset(offset as isize)) as *mut i32) = num
                    };
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            KVM_EXIT_INTERNAL_ERROR => {
                println!("KVM_EXIT_INTERNAL_ERROR");
                break;
            }
            KVM_EXIT_SHUTDOWN => {
                println!("KVM_EXIT_SHUTDOWN");
                break;
            }
            _ => {
                println!("Unsupported exit_reason: {}", exit_reason);
                println!("Abort!");
                break;
            }
        };
    }
}
