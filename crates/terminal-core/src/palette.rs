pub(crate) const DEFAULT_ANSI_COLORS: [[u8; 3]; 16] = [
    [0, 0, 0],
    [205, 49, 49],
    [13, 188, 121],
    [229, 229, 16],
    [36, 114, 200],
    [188, 63, 188],
    [17, 168, 205],
    [229, 229, 229],
    [102, 102, 102],
    [241, 76, 76],
    [35, 209, 139],
    [245, 245, 67],
    [59, 142, 234],
    [214, 112, 214],
    [41, 184, 219],
    [255, 255, 255],
];

/// Returns the default ANSI or fixed xterm 256-color palette entry.
pub fn default_indexed_color(index: u8) -> [u8; 3] {
    if index < 16 {
        return DEFAULT_ANSI_COLORS[usize::from(index)];
    }
    if index >= 232 {
        return [8 + (index - 232) * 10; 3];
    }
    let index = usize::from(index - 16);
    let levels = [0, 95, 135, 175, 215, 255];
    [
        levels[index / 36],
        levels[(index / 6) % 6],
        levels[index % 6],
    ]
}
