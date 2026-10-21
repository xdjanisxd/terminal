//! Bounded UTF-8 scratch space for incremental extended grapheme boundaries.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Cell, MAX_GRAPHEME_SCALARS};

/// Tests the existing cell plus one scalar without allocation. The extra slot
/// permits checking a boundary even when the cell payload is already full.
pub(crate) fn extended_width(cell: &Cell, next: char) -> Option<usize> {
    let mut bytes = [0; (MAX_GRAPHEME_SCALARS + 1) * 4];
    let mut length = 0;
    for character in std::iter::once(cell.character())
        .chain(cell.grapheme_tail().iter().copied())
        .chain(std::iter::once(next))
    {
        length += character.encode_utf8(&mut bytes[length..]).len();
    }
    let text = std::str::from_utf8(&bytes[..length]).expect("encoded scalars are UTF-8");
    (text.graphemes(true).next()?.len() == length).then(|| text.width())
}
