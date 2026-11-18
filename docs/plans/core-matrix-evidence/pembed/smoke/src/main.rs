// SPDX-License-Identifier: MIT OR Apache-2.0
//! P-EMBED S4 smoke link: bump allocator + panic handler, constructs a
//! GF2Matrix (gf2 path) and a VerifiedRotor (phantom path), links via
//! rust-lld. Requirements documented in the contract: consumers provide
//! a global allocator for alloc-dependent APIs; eabihf hard-float ABI;
//! board/runtime support comes from the consumer's ecosystem
//! (amari-core is a library, not a board crate).

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

static HEAP_SIZE: usize = 16 * 1024;
static mut HEAP: [u8; 16 * 1024] = [0; 16 * 1024];
static USED: AtomicUsize = AtomicUsize::new(0);

/// Minimal bump allocator (smoke-grade; NOT production advice).
struct BumpAlloc;

unsafe impl Sync for BumpAlloc {}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let start = HEAP.as_mut_ptr() as usize;
        let align = layout.align();
        let used = USED.load(Ordering::Relaxed);
        let next = (start + used + align - 1) & !(align - 1);
        let end = next + layout.size();
        if end > start + HEAP_SIZE {
            return core::ptr::null_mut();
        }
        USED.store(end - start, Ordering::Relaxed);
        next as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    // GF(2) path: identity matrix + rank.
    let eye = amari_core::gf2::GF2Matrix::identity(4);
    let _rank = eye.rank();

    // Phantom path: the unit bivector rotor e1∧e2. Cl(0,2,0) has
    // 2^2 = 4 basis blades (R1 P2: the first draft supplied two
    // coefficients, which VerifiedMultivector::new rejects).
    use amari_core::verified::{VerifiedMultivector, VerifiedRotor};
    let coeffs = alloc::vec![0.0f64, 0.0, 0.0, 1.0];
    let mv = VerifiedMultivector::<f64, 0, 2, 0>::new(coeffs)
        .ok()
        .expect("unit bivector");
    let rotor = VerifiedRotor::<f64, 0, 2, 0>::new(mv)
        .ok()
        .expect("rotor invariant");

    // Touch the rotor so the consumer path is real (compose with itself).
    let _ = rotor.compose(&rotor).ok().expect("composition");
    0
}
