// SPDX-License-Identifier: GPL-3.0-or-later
//! The `text.bmp` font: 5 x 6 cells in three rows, as Winamp picks a cell for a character.

/// The cell of `c` in `text.bmp`: its left and top. Letters are one row, case folded; digits
/// and punctuation the second; a few more marks the third. Accented letters take their base
/// letter; anything else is the blank cell.
pub fn cell(c: char) -> (i32, i32) {
    let c = fold(c);
    if c.is_ascii_uppercase() {
        return ((c as i32 - 'A' as i32) * 5, 0);
    }
    if c.is_ascii_lowercase() {
        return ((c as i32 - 'a' as i32) * 5, 0);
    }
    let row1 = |i: i32| (i * 5, 6);
    let row2 = |i: i32| (i * 5, 12);
    match c {
        '\u{1}' => row1(10),
        '0'..='9' => row1(c as i32 - '0' as i32),
        '.' => row1(11),
        ':' => row1(12),
        '(' => row1(13),
        ')' => row1(14),
        '-' => row1(15),
        '\'' | '`' => row1(16),
        '!' => row1(17),
        '_' => row1(18),
        '+' => row1(19),
        '\\' => row1(20),
        '/' => row1(21),
        '[' | '{' | '<' => row1(22),
        ']' | '}' | '>' => row1(23),
        '~' | '^' => row1(24),
        '&' => row1(25),
        '%' => row1(26),
        ',' => row1(27),
        '=' => row1(28),
        '$' => row1(29),
        '#' => row1(30),
        'Å' | 'å' => row2(0),
        'Ö' | 'ö' => row2(1),
        'Ä' | 'ä' => row2(2),
        '?' => row2(3),
        '*' => row2(4),
        '"' => (26 * 5, 0),
        '@' => (27 * 5, 0),
        _ => (30 * 5, 0),
    }
}

/// The blank cell: where a space goes.
pub const BLANK: (i32, i32) = (150, 0);

/// An accented letter as its base letter, as Winamp folds them before picking a cell.
fn fold(c: char) -> char {
    match c {
        'Æ' => 'A',
        'Ç' => 'C',
        'É' => 'E',
        'Ñ' | 'ñ' => 'n',
        'Ü' => 'U',
        'à'..='â' | 'æ' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ò'..='ô' => 'o',
        'ù'..='û' | 'ü' => 'u',
        'ÿ' => 'y',
        'ƒ' => 'f',
        '°' => '0',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::cell;

    #[test]
    fn letters_digits_and_marks_take_their_cells() {
        assert_eq!(cell('A'), (0, 0));
        assert_eq!(cell('z'), (125, 0));
        assert_eq!(cell('0'), (0, 6));
        assert_eq!(cell('9'), (45, 6));
        assert_eq!(cell('.'), (55, 6));
        assert_eq!(cell(':'), (60, 6));
        assert_eq!(cell('('), (65, 6));
        assert_eq!(cell(')'), (70, 6));
        assert_eq!(cell('-'), (75, 6));
        assert_eq!(cell('?'), (15, 12));
        assert_eq!(cell('*'), (20, 12));
        assert_eq!(cell('"'), (130, 0));
        assert_eq!(cell('@'), (135, 0));
        assert_eq!(cell(' '), (150, 0));
        assert_eq!(cell('é'), cell('e'));
    }
}
