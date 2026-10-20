# Selection edge autoscroll

While holding the left mouse button to extend a terminal text selection, keep
the pointer near the top or bottom of the active pane to scroll toward older
history or live output. The edge zones span up to two cell heights, with a
neutral middle third of the text viewport retained in short panes. The last
partial row/column of pane space participates in dragging and maps to the last
terminal cell.

Speed increases with pointer penetration into the edge zone. The quadratic
curve keeps fine control near the inner edge and reaches maximum speed at the
text boundary (including partial pane pixels beyond the last text row):

| Pointer position in the zone | Old speed | New speed |
| --- | --- | --- |
| Inner boundary | 20 rows/s | 12 rows/s |
| One quarter toward the edge | 20 rows/s | 86.25 rows/s |
| Halfway toward the edge | 20 rows/s | 309 rows/s |
| Three quarters toward the edge | 20 rows/s | 680.25 rows/s |
| At/beyond the text boundary, inside the pane | 20 rows/s | 1,200 rows/s |

Previously one row moved every 50 ms. The existing event-loop scheduler now
checks at most once every 16 ms (62.5 updates/s). It integrates elapsed time at
the current speed, retains fractional rows, and moves whole rows in one batch
with one selection extension and one redraw invalidation. Pointer motion does
not restart the deadline or multiply the speed. Ordinary wake/render frequency
does not determine distance traveled. Time after a stall is capped at 50 ms and
pending movement at 60 rows, with no catch-up debt. Changing direction resets
fractional movement so the previous direction cannot leak into a reversal.

At maximum speed, 5,000 rows take about 4.17 seconds rather than 250 seconds,
assuming timely event-loop wakes. These are nominal rates; history bounds and
busy-event-loop caps can reduce the distance. No extra work runs while inactive:
there is no autoscroll deadline, and the tick returns immediately. At either
history bound the deadline is removed as soon as the bound is reached; moving
the pointer in the edge zone can arm it again.

Moving inward, leaving the active pane/window, releasing the button, losing
focus, changing panes, or ending the selection stops autoscroll. Mouse tracking
belongs to the application unless Shift is held for terminal selection. Alternate
Screen permits ordinary selection but has no history autoscroll. Geometry,
selection availability, and mouse ownership are checked again before timed
scrolling. Selection anchors, soft-wrap joining, reflow, and copy remain owned
by `terminal-core`.

## Root cause and implementation

The old input path armed only on the first/last grid row and required an exact
terminal-cell hit. Moving into the remaining pixels below the grid cancelled
scrolling, while a fast drag could skip the narrow activation row. Timed wakes
reused cached row/column and drag state without revalidating geometry, selection
availability, or application mouse ownership. Existing core tests already
demonstrated that scrolling and extending the selection worked together.

`crates/app/src/selection_scroll.rs` repairs that app path using the existing
`selection_edge` deadline and `scroll_selection_viewport_rows` operation. No
background timer, thread, new dependency, or terminal selection model is added.
Only an armed drag requests deadline wakes; an unsuccessful scroll clears it.

The speed follow-up replaces the fixed one-row-per-wake policy with elapsed-time
accumulation and a distance-based rate in that same scheduler. A named state
stores the rate, fractional movement and timestamps; terminal-core still applies
the bounded viewport movement and selection extension.

## Verification

The initial correctness repair passed workspace validation (834 passed, 27
ignored), formatting, workspace all-target checking, Clippy and a release build.
The subsequent adaptive-speed change uses focused app validation instead of
repeating the full workspace suite.

Focused regressions cover partial cells, pane boundaries, DPI-scaled zones,
short-pane neutral space, stationary scrolling both ways, speed/deadlines,
history bounds, leaving zones/panes, release, selection end, modal input,
mouse tracking with Shift override, Alternate Screen, and copy through reflow.

Adaptive-speed Linux validation:

- App unit tests: 201 passed, 7 ignored; focused autoscroll regressions: 11 passed.
- Formatting, app all-target Clippy with warnings denied, and diff checks passed.
  The release-mode performance test compiled and passed; the full workspace
  suite was not repeated for the adaptive-speed follow-up.
- Batched scrolling produces the same copied text as repeated single-row
  scrolling across soft wraps, wide characters, and reflow.
- Simulated 16, 20 and 40 ms wakes, with and without extra pointer events,
  all move 2,400 rows in two seconds at maximum speed (within one row).
- A release-mode scheduler performance check moves 4,800 rows in four simulated
  seconds with exactly 250 redraw invalidations. One million inactive calls
  produce zero invalidations and no deadline. This measures scheduler/core CPU
  cost and requested redraws, not native GPU time or manual smoothness.
  One local release sample took 12.019 microseconds for the 250 active ticks and
  692.232 microseconds for the million inactive calls (elapsed loop times, not
  end-to-end rendering measurements).

The working tree was clean before the initial repair; the adaptive-speed work
preserves those uncommitted changes on the same branch. At the initial repair,
the existing requested branch pointed at the freshly fetched `origin/master`
(`bab410a`), so it was retained. No commit or push was made.

## Linux and Windows manual acceptance

Run the following on Linux KDE Wayland and Windows with the release binary.
These are manual acceptance steps, not claims of completed native validation.

1. Generate numbered lines: Linux `seq 1 10000`; PowerShell `1..10000`.
   Start a selection in the middle of the pane. Drag into the top two rows,
   hold just inside the zone and confirm precise movement. Move halfway toward
   the edge and then onto the boundary: speed should rise smoothly to roughly
   1,200 rows/s, traversing thousands of rows in seconds. Hold still to confirm
   continuous selection growth. Reverse into the bottom zone and confirm the
   same acceleration and selection extension back to live output. The partial
   pixels below the last text row should run at maximum speed; crossing the
   pane boundary must still stop scrolling.
2. Move inward, across a pane divider, into title/tab chrome, and outside the
   window. Confirm scrolling stops for each. Re-enter the original pane edge
   while holding the button and confirm it resumes. Release at either edge;
   confirm the copied selection remains stable and scrolling stops.
3. Reach both ends of history. Confirm scrolling stops without repeated redraws
   or sustained CPU use. Leave the selection inactive and confirm normal idle
   behavior. Move the pointer in the edge zone to restart where history permits.
4. Select a long soft-wrapped line across history, resize narrower/wider during
   the drag, release and copy with Ctrl+Shift+C. Confirm characters and hard
   breaks match the source, with no extra newlines at soft wraps. Repeat word
   and line selections (double/triple click).
5. Repeat with horizontal and vertical splits, a short pane, fractional DPI,
   and a window height not divisible by cell height. Confirm bottom partial
   pixels work and neighboring panes never scroll. Change focus/tab/pane or
   open a palette during a drag; confirm the old drag stops.
6. In a mouse-reporting application, confirm normal drags reach the application.
   Hold Shift for terminal selection; confirm history scrolling only on Primary.
   Release Shift while at an edge; confirm autoscroll stops. In an Alternate
   Screen application, confirm neither edge scrolls Primary history.

Native manual acceptance is pending on both platforms. Windows compilation and
runtime validation require a Windows environment; this checkout has only the
Linux Rust target installed.
