//! Expose the target interpreter's configuration to the extension's source.
//!
//! The package ships one abi3 wheel per platform. Free-threaded CPython cannot
//! load abi3 extensions, and PyO3 would otherwise fall back to a silent
//! version-specific build that reenables the GIL on import; `lib.rs` refuses
//! that configuration instead.
fn main() {
    pyo3_build_config::use_pyo3_cfgs();
}
