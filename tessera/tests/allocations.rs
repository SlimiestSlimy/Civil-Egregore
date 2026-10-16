//! Encoding and decoding never allocate: a `Tessera`, a stream and a
//! bitmap made once, then every bitmap of a corpus encoded and decoded
//! through them -- the first included -- with every allocation this
//! thread makes counted by a global allocator of the test's own.
//!
//! `cargo test --test allocations`

use bitmap::Bitmap;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use tessera::diagnostics::adversarial::worst;
use tessera::corpus::checkerboards::one_checkerboard;
use tessera::corpus::{corpus_families, HowMany};
use tessera::{BitStream, Tessera};

/// The system allocator, counting the allocations of a thread that asks.
struct CountingAllocator;

thread_local! {
    /// Whether this thread's allocations are counted, and how many so far.
    static COUNTED_ALLOCATIONS: Cell<(bool, usize)> = const { Cell::new((false, 0)) };
}

/// Counts an allocation, if this thread is counting.
fn count() {
    COUNTED_ALLOCATIONS.with(|counted_allocations| {
        let (is_counting, allocations) = counted_allocations.get();
        if is_counting {
            counted_allocations.set((true, allocations + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, allocation_pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(allocation_pointer, layout, new_size) }
    }

    unsafe fn dealloc(&self, allocation_pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(allocation_pointer, layout) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

/// Every family's tested bitmaps, a checkerboard and the saved
/// adversarial bitmaps, encoded and decoded with no allocation at all.
#[test]
fn encoding_and_decoding_never_allocate() {
    let mut corpus: Vec<Bitmap> = corpus_families(HowMany::TestedCount).into_iter().flat_map(|(_, bitmaps)| bitmaps).collect();
    corpus.push(one_checkerboard(7));
    corpus.extend(worst::every_saved().into_iter().map(|(_, bitmap)| bitmap));
    let (mut tessera, mut stream, mut decoded_bitmap) = (Tessera::new(), BitStream::default(), Bitmap::new());
    COUNTED_ALLOCATIONS.with(|counted_allocations| counted_allocations.set((true, 0)));
    for bitmap in &corpus {
        tessera.encode(bitmap, &mut stream);
        tessera.decode(&stream, &mut decoded_bitmap);
    }
    let (_, allocations) = COUNTED_ALLOCATIONS.with(|counted_allocations| counted_allocations.replace((false, 0)));
    assert_eq!(allocations, 0, "{allocations} allocations encoding and decoding {} bitmaps", corpus.len());
}
