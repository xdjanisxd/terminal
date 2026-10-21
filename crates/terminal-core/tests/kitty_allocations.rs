use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
use terminal_core::*;
struct CountingAllocator;
thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1))
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = COUNT.try_with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1))
            }
        });
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[test]
fn ordinary_reports_and_negotiation_do_not_allocate() {
    let mut terminal = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
    let mut parser = TerminalParser::new();
    COUNT.with(|c| c.set(Some(0)));
    for _ in 0..100 {
        parser.advance(&mut terminal, b"\x1b[>31u\x1b[?u").unwrap();
        std::hint::black_box(terminal.take_reply().unwrap().as_bytes());
        for key in [
            KeyboardKey::Character('a'),
            KeyboardKey::Character('界'),
            KeyboardKey::Functional(57352),
        ] {
            for kind in [
                KeyboardEventKind::Press,
                KeyboardEventKind::Repeat,
                KeyboardEventKind::Release,
            ] {
                let result = encode_kitty_key(
                    *terminal.input_modes(),
                    KeyboardEvent {
                        key,
                        shifted: Some('A'),
                        base_layout: Some('a'),
                        modifiers: KeyboardModifiers(5),
                        kind,
                        text: Some("A"),
                    },
                )
                .unwrap();
                std::hint::black_box(result.as_slice());
            }
        }
        parser.advance(&mut terminal, b"\x1b[<u").unwrap();
    }
    let allocations = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(allocations, 0);
}

#[test]
fn ascii_and_grapheme_printing_do_not_allocate_per_cell() {
    let mut terminal = TerminalState::new(TerminalDimensions::new(80, 24).unwrap());
    let mut parser = TerminalParser::new();
    COUNT.with(|c| c.set(Some(0)));
    for _ in 0..100 {
        parser
            .advance(&mut terminal, "\rASCII e\u{301} 👩‍💻 🇺🇸 👍🏽 1️⃣ 👨‍👩‍👧‍👦".as_bytes())
            .unwrap();
    }
    let allocations = COUNT.with(|c| c.replace(None).unwrap());
    assert_eq!(allocations, 0);
}
