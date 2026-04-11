//! Benchmark: how fast is project detection?
//!
//! Run with: cargo bench

use o8v_project::ProjectRoot;
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let root =
        ProjectRoot::new(env!("CARGO_MANIFEST_DIR")).expect("benchmark directory must exist");

    // Warmup
    for _ in 0..10 {
        black_box(o8v_project::detect_all(black_box(&root)));
    }

    // Measure
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(o8v_project::detect_all(black_box(&root)));
    }
    let elapsed = start.elapsed();
    let per_call = elapsed / iterations;
    println!("detect_all() × {iterations}");
    println!("  total:    {elapsed:?}");
    println!("  per call: {per_call:?}");

    // Empty directory
    let empty = tempfile::tempdir().expect("must create temp dir");
    let empty_root = ProjectRoot::new(empty.path()).expect("temp dir must be valid");

    let start = Instant::now();
    for _ in 0..iterations {
        black_box(o8v_project::detect_all(black_box(&empty_root)));
    }
    let elapsed = start.elapsed();
    let per_call = elapsed / iterations;
    println!("\ndetect_all() on empty dir × {iterations}");
    println!("  total:    {elapsed:?}");
    println!("  per call: {per_call:?}");
}
