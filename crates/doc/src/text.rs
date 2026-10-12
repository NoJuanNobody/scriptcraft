//! Char-boundary and word helpers on byte offsets.

/// The char boundary before `off` (0 at the start).
pub fn prev_boundary(s: &str, off: usize) -> usize {
    let mut i = off.min(s.len());
    if i == 0 {
        return 0;
    }
    i -= 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The char boundary after `off` (`s.len()` at the end).
pub fn next_boundary(s: &str, off: usize) -> usize {
    let mut i = off.min(s.len());
    if i >= s.len() {
        return s.len();
    }
    i += 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '’'
}

/// The start of the word before `off` (skipping spaces and punctuation first).
pub fn word_left(s: &str, off: usize) -> usize {
    let mut i = off.min(s.len());
    while i > 0 {
        let p = prev_boundary(s, i);
        if s.get(p..i).and_then(|c| c.chars().next()).is_some_and(is_word) {
            break;
        }
        i = p;
    }
    while i > 0 {
        let p = prev_boundary(s, i);
        if !s.get(p..i).and_then(|c| c.chars().next()).is_some_and(is_word) {
            break;
        }
        i = p;
    }
    i
}

/// The end of the word after `off` (skipping spaces and punctuation first).
pub fn word_right(s: &str, off: usize) -> usize {
    let mut i = off.min(s.len());
    while i < s.len() {
        let n = next_boundary(s, i);
        if s.get(i..n).and_then(|c| c.chars().next()).is_some_and(is_word) {
            break;
        }
        i = n;
    }
    while i < s.len() {
        let n = next_boundary(s, i);
        if !s.get(i..n).and_then(|c| c.chars().next()).is_some_and(is_word) {
            break;
        }
        i = n;
    }
    i
}
