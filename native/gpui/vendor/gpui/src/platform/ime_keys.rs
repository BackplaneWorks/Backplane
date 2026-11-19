//! Windows virtual keys that belong to the input method (Backplane patch, PATCH.md). Plain numbers and no platform types, so
//! the client tests the mapping on any host (`gpui::is_windows_ime_vkey`).

/// Whether a Windows virtual-key code is an IME key: kana/hangul, IME on/off, junja, final, kanji/hanja, convert,
/// nonconvert, accept, mode change, `VK_PROCESSKEY`, and the Japanese OEM keys (Eisu, Katakana, Hiragana,
/// Hankaku, Zenkaku, backtab). Such a key is never an app shortcut: it goes to `TranslateMessage` (so the IME sees it) and
/// to `DefWindowProc`, whether or not it maps to a keystroke.
pub fn is_windows_ime_vkey(vkey: u16) -> bool {
    matches!(vkey, 0x15..=0x1A | 0x1C..=0x1F | 0xE5 | 0xF0..=0xF5)
}

/// The span of `units` (UTF-16) to reconvert: the selection `start..end`, or with no selection the text before the caret back to
/// the previous line break, at most 16 units.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn reconvert_span(units: &[u16], start: usize, end: usize) -> (usize, usize) {
    let (start, end) = (start.min(units.len()), end.min(units.len()));
    if start < end {
        return (start, end);
    }
    let floor = start.saturating_sub(16);
    let from = units[floor..start].iter().rposition(|c| *c == 10 || *c == 13).map_or(floor, |i| floor + i + 1);
    (from, start)
}

#[cfg(test)]
mod tests {
    use super::is_windows_ime_vkey as ime;

    #[test]
    fn japanese_ime_keys_are_ime_keys() {
        for (name, vk) in [
            ("VK_KANA", 0x15),
            ("VK_IME_ON", 0x16),
            ("VK_KANJI (Hankaku/Zenkaku)", 0x19),
            ("VK_IME_OFF", 0x1A),
            ("VK_CONVERT (Henkan)", 0x1C),
            ("VK_NONCONVERT (Muhenkan)", 0x1D),
            ("VK_ACCEPT", 0x1E),
            ("VK_MODECHANGE", 0x1F),
            ("VK_PROCESSKEY", 0xE5),
            ("VK_OEM_ATTN (Eisu)", 0xF0),
            ("VK_OEM_FINISH (Katakana)", 0xF1),
            ("VK_OEM_COPY (Hiragana)", 0xF2),
            ("VK_OEM_AUTO (Hankaku)", 0xF3),
            ("VK_OEM_ENLW (Zenkaku)", 0xF4),
            ("VK_OEM_BACKTAB", 0xF5),
        ] {
            assert!(ime(vk), "{name} must go to the IME");
        }
    }

    #[test]
    fn reconvert_selection_or_text_before_caret() {
        let t: Vec<u16> = "abc\nこんにちは".encode_utf16().collect();
        assert_eq!(super::reconvert_span(&t, 4, 7), (4, 7));
        assert_eq!(super::reconvert_span(&t, 9, 9), (4, 9));
        assert_eq!(super::reconvert_span(&t, 0, 0), (0, 0));
        assert_eq!(super::reconvert_span(&t, 99, 99).0 <= 9, true);
    }

    #[test]
    fn ordinary_keys_are_not() {
        for vk in [0x08, 0x09, 0x0D, 0x10, 0x11, 0x12, 0x14, 0x1B, 0x14, 0x20, 0x30, 0x41, 0x5A, 0x70, 0xBA, 0xE2, 0xE6, 0xF6] {
            assert!(!ime(vk), "{vk:#x} is an ordinary key");
        }
    }
}
