use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static ALLOCATOR: WasmAllocator = WasmAllocator;

struct WasmAllocator;

static mut HEAP_END: usize = 0;

unsafe impl GlobalAlloc for WasmAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        if HEAP_END == 0 {
            let pages = core::arch::wasm32::memory_size(0);
            HEAP_END = pages * 65536;
        }

        let aligned = (HEAP_END + align - 1) & !(align - 1);
        let new_end = aligned + size;

        let current_pages = core::arch::wasm32::memory_size(0);
        let available = current_pages * 65536;

        if new_end > available {
            let needed_pages = (new_end - available + 65535) / 65536;
            let result = core::arch::wasm32::memory_grow(0, needed_pages);
            if result == usize::MAX {
                return core::ptr::null_mut();
            }
        }

        HEAP_END = new_end;
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
