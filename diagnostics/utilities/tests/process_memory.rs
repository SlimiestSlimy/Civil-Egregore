//! The process's memory: read, and tracked over a run.
//!
//! `cargo test`

use utilities::diagnostics::process_memory::{process_memory, MemoryTrack};

/// On Linux the process holds some memory, never more than its peak, and
/// a track of it averages between nothing and the peak.
#[test]
fn memory_is_read_and_tracked() {
    let Some(memory_reading) = process_memory() else {
        return;
    };
    assert!(memory_reading.resident_bytes > 0 && memory_reading.resident_bytes <= memory_reading.peak_bytes);
    let mut memory_track = MemoryTrack::default();
    // Kept from being optimized away, so the memory is really held.
    let kept_buffer = std::hint::black_box(vec![1u8; 32 << 20]);
    memory_track.take_reading();
    drop(kept_buffer);
    memory_track.take_reading();
    let (average_resident, tracked_peak) = (memory_track.average_resident_bytes().expect("read"), memory_track.peak_bytes().expect("read"));
    assert!(average_resident > 0 && average_resident <= tracked_peak && tracked_peak >= 32 << 20);
}
