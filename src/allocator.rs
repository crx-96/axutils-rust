#[cfg(all(feature = "mimalloc", feature = "rpmalloc"))]
compile_error!(
    "axutils_allocator_conflict: enable only one global allocator backend: `mimalloc` or `rpmalloc`"
);

#[cfg(all(feature = "mimalloc", not(feature = "rpmalloc")))]
#[global_allocator]
static AXUTILS_MIMALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "rpmalloc", not(feature = "mimalloc")))]
#[global_allocator]
static AXUTILS_RPMALLOC: rpmalloc::RpMalloc = rpmalloc::RpMalloc;

// `rpmalloc` unconditionally enables the `rpmalloc-sys` `preload` feature, and
// the Windows C source references token-management APIs from Advapi32 without
// emitting the corresponding Cargo link directive. Carry the native library
// link through this crate so a downstream binary can link the selected
// allocator.
#[cfg(all(feature = "rpmalloc", target_os = "windows"))]
#[link(name = "advapi32")]
unsafe extern "system" {}
