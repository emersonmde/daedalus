# Heap Allocator

DaedalusOS uses a simple bump allocator for heap memory management, providing dynamic allocation capabilities for Rust's `alloc` crate.

## Overview

The bump allocator is the simplest form of memory allocator:
- Maintains a pointer that "bumps" forward with each allocation
- Individual deallocations are no-ops
- Memory is only reclaimed when the entire allocator is reset
- Fast O(1) allocation time
- Minimal overhead and complexity

This design is ideal for kernel workloads where:
- Memory is frequently allocated but rarely freed individually
- Simple, predictable behavior is preferred over complex memory management
- Performance and code size matter more than memory reuse

## Implementation

**Location**: `src/mm/allocator.rs`

The `BumpAllocator` struct implements:
- `GlobalAlloc` trait for Rust's standard allocator interface
- Thread-safe access using `spin::Mutex`
- Proper alignment handling for all allocations
- Memory tracking (total size, used, free)

### Memory Layout

```
Heap Start (0x00880000)                          Heap End (0x01080000)
    |                                                        |
    v                                                        v
    [=============== Allocated ===============][=== Free ===]
                                               ^
                                               |
                                            next pointer
```

The allocator manages an 8 MB region defined by linker symbols:
- `__heap_start`: Beginning of heap region (after BSS section)
- `__heap_end`: End of heap region (before stack)
- `next`: Current allocation pointer (bumps forward)

## Initialization

The heap is initialized during kernel startup in `lib.rs::init()`:

```rust
unsafe {
    extern "C" {
        static __heap_start: u8;
        static __heap_end: u8;
    }
    let heap_start = &__heap_start as *const u8 as usize;
    let heap_end = &__heap_end as *const u8 as usize;
    ALLOCATOR.init(heap_start, heap_end);
}
```

### Safety Invariants

The allocator relies on several safety invariants:
1. `init()` is called exactly once before any allocations
2. The heap region `[heap_start, heap_end)` is valid, properly aligned memory
3. The region is reserved exclusively for heap use (no overlap with code/stack)
4. `heap_start < heap_end` (enforced by linker script)

Debug assertions catch configuration errors during development.

## Allocation Strategy

### Alignment

All allocations are properly aligned according to the requested `Layout`:

```rust
let alloc_start = (next + layout.align() - 1) & !(layout.align() - 1);
```

This ensures that returned pointers meet ARM AAPCS alignment requirements.

### Out of Memory

When the heap is exhausted:
1. `alloc()` returns a null pointer
2. Rust's allocator calls the `#[alloc_error_handler]`
3. The kernel panics with allocation error details

### Deallocation

The bump allocator does **not** free individual allocations:

```rust
unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
    // No-op: bump allocator never deallocates individual allocations
}
```

This is a deliberate design choice that trades memory efficiency for simplicity and speed.

## Usage Examples

The allocator enables Rust's standard collections:

```rust
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;

// Heap-allocated value
let value = Box::new(42);

// Dynamic array
let mut vec = Vec::new();
vec.push(1);
vec.push(2);

// Owned string
let mut s = String::from("Hello");
s.push_str(", World!");
```

### Shell History

The interactive shell uses the allocator for command history:

```rust
static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn handle_line(line: &str) {
    let mut history = HISTORY.lock();
    history.push(String::from(line));
}
```

## Monitoring

The allocator provides runtime statistics:

```rust
ALLOCATOR.heap_size()  // Total heap capacity (8 MB)
ALLOCATOR.used()       // Bytes allocated so far
ALLOCATOR.free()       // Bytes remaining
```

These are exposed through the `meminfo` shell command.

## Testing

The allocator is tested in `src/lib.rs` with:
- `test_box_allocation` - Box allocation
- `test_vec_allocation` - Vec creation and push
- `test_string_allocation` - String concatenation
- `test_vec_with_capacity` - Pre-allocated capacity
- `test_allocator_stats` - Usage tracking

All tests run in QEMU during `cargo test`.

## Future Improvements

Potential enhancements for later phases:
- **Free list allocator** - Reuse deallocated memory
- **Slab allocator** - Fixed-size pools for common allocations
- **Per-CPU allocators** - Reduce contention in SMP
- **Memory pressure callbacks** - Allow cleanup when low on memory

For now, the bump allocator provides a solid foundation for Phase 2 development.

## References

- **Code**: `src/mm/allocator.rs` (164 lines)
- **Linker symbols**: `linker.ld` defines `__heap_start` and `__heap_end`
- **Integration**: `src/lib.rs` - initialization and global allocator registration
- **Rust allocator API**: [GlobalAlloc trait documentation](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html)
