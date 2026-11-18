#![cfg(feature = "aes")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use axutils::crypto::{AesCipher, AesMode};

const PAYLOAD_LEN: usize = 128 * 1024;
static PAYLOAD_SIZED_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

fn count_payload_buffer(size: usize) {
    if size >= PAYLOAD_LEN {
        PAYLOAD_SIZED_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

// 此测试独占集成测试进程，仅统计足以容纳整个输入的分配和扩容。
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_payload_buffer(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_payload_buffer(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[test]
fn random_iv_encryption_allocates_one_payload_sized_buffer() {
    let plaintext = vec![0x42; PAYLOAD_LEN];
    for mode in [AesMode::CbcPkcs7, AesMode::Gcm] {
        let cipher = AesCipher::from_key_bytes([0x11; 32], mode).unwrap();
        PAYLOAD_SIZED_ALLOCATIONS.store(0, Ordering::Relaxed);
        let container = cipher.encrypt(&plaintext).unwrap();
        assert_eq!(
            PAYLOAD_SIZED_ALLOCATIONS.load(Ordering::Relaxed),
            1,
            "{mode:?}"
        );
        assert_eq!(cipher.decrypt(container).unwrap(), plaintext);
    }
}
