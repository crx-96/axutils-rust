use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        System.realloc(pointer, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[cfg(any(feature = "itoa", feature = "ryu", feature = "zmij", feature = "uuid"))]
fn allocations_during<F>(operation: F) -> usize
where
    F: FnOnce(),
{
    ALLOCATIONS.store(0, Ordering::Relaxed);
    operation();
    ALLOCATIONS.load(Ordering::Relaxed)
}

#[test]
fn borrowed_and_preallocated_append_paths_do_not_allocate_extra_results() {
    #[cfg(feature = "itoa")]
    {
        use axutils::{convert::IntegerBuffer, utils::ConvertUtils};

        let mut buffer = IntegerBuffer::new();
        let _ = ConvertUtils::integer_to_str(i64::MIN, &mut buffer);
        let borrowed_allocations = allocations_during(|| {
            let text = ConvertUtils::integer_to_str(i64::MAX, &mut buffer);
            std::hint::black_box(text);
        });
        assert_eq!(borrowed_allocations, 0);

        let mut output = String::with_capacity(64);
        ConvertUtils::append_integer(&mut output, 1_i128);
        output.clear();
        let append_allocations = allocations_during(|| {
            ConvertUtils::append_integer(&mut output, i128::MIN);
            std::hint::black_box(&output);
        });
        assert_eq!(append_allocations, 0);
    }

    #[cfg(any(feature = "ryu", feature = "zmij"))]
    {
        use axutils::{
            convert::{FloatBuffer, FloatFormat},
            utils::ConvertUtils,
        };

        #[cfg(feature = "ryu")]
        let format = FloatFormat::Ryu;
        #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
        let format = FloatFormat::Zmij;

        let mut buffer = FloatBuffer::new(format);
        let _ = ConvertUtils::float_to_str(1.25_f64, &mut buffer);
        let borrowed_allocations = allocations_during(|| {
            let text = ConvertUtils::float_to_str(-1.25_f64, &mut buffer);
            std::hint::black_box(text);
        });
        assert_eq!(borrowed_allocations, 0);

        let mut output = String::with_capacity(64);
        ConvertUtils::append_float(&mut output, 1.25_f64, format);
        output.clear();
        let append_allocations = allocations_during(|| {
            ConvertUtils::append_float(&mut output, -1.25_f64, format);
            std::hint::black_box(&output);
        });
        assert_eq!(append_allocations, 0);
    }

    #[cfg(feature = "uuid")]
    {
        use axutils::{convert::UuidBuffer, utils::ConvertUtils};

        let uuid = ConvertUtils::string_to_uuid("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let mut buffer = UuidBuffer::new();
        let _ = ConvertUtils::uuid_to_str(&uuid, &mut buffer);
        let borrowed_allocations = allocations_during(|| {
            let text = ConvertUtils::uuid_to_str(&uuid, &mut buffer);
            std::hint::black_box(text);
        });
        assert_eq!(borrowed_allocations, 0);

        let mut output = String::with_capacity(64);
        ConvertUtils::append_uuid(&mut output, &uuid);
        output.clear();
        let append_allocations = allocations_during(|| {
            ConvertUtils::append_uuid(&mut output, &uuid);
            std::hint::black_box(&output);
        });
        assert_eq!(append_allocations, 0);
    }
}
