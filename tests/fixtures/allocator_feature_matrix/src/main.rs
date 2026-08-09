#[cfg(any(
    feature = "mimalloc-downstream-system",
    feature = "rpmalloc-downstream-system"
))]
#[global_allocator]
static DOWNSTREAM_SYSTEM: std::alloc::System = std::alloc::System;

#[repr(align(64))]
struct AlignedBytes([u8; 64]);

#[cfg(any(feature = "mimalloc-serde", feature = "rpmalloc-serde"))]
fn exercise_serde_api() {
    use axutils::{ConfigFormat, ConfigUtils};

    let value = ConfigUtils::parse_value(r#"{"allocator":true}"#, ConfigFormat::Json)
        .expect("fixture serde configuration");
    assert!(value.get("allocator").and_then(|item| item.as_bool()) == Some(true));
}

#[cfg(not(any(feature = "mimalloc-serde", feature = "rpmalloc-serde")))]
fn exercise_serde_api() {}

fn main() {
    use std::alloc::{alloc, dealloc, Layout};

    exercise_serde_api();

    let current_dir = axutils::PathUtils::current_dir().expect("fixture current directory");
    assert!(!current_dir.as_os_str().is_empty());

    let mut values = Vec::with_capacity(8);
    values.extend(0u8..8);
    values.reserve(256);
    values.extend(8u8..=255);
    assert_eq!(values.len(), 256);
    assert_eq!(values[255], 255);

    let boxed_values = Box::new(values);
    assert_eq!(boxed_values.first(), Some(&0));
    assert_eq!(boxed_values.last(), Some(&255));

    let thread_values = std::thread::spawn(|| {
        let mut values = Vec::with_capacity(32);
        values.extend(0u8..128);
        values
    })
    .join()
    .expect("fixture worker thread");
    assert_eq!(thread_values.len(), 128);
    assert_eq!(thread_values[127], 127);

    let aligned = Box::new(AlignedBytes([0xA5; 64]));
    let address = (&*aligned as *const AlignedBytes) as usize;
    assert_eq!(address % std::mem::align_of::<AlignedBytes>(), 0);
    assert_eq!(aligned.0[63], 0xA5);

    let layout = Layout::from_size_align(128, 64).expect("fixture allocation layout");
    let pointer = unsafe { alloc(layout) };
    assert!(!pointer.is_null());
    unsafe {
        pointer.write_bytes(0x5A, layout.size());
        assert_eq!(*pointer.add(layout.size() - 1), 0x5A);
        dealloc(pointer, layout);
    }

    println!("axutils_allocator_fixture_ok");
}
