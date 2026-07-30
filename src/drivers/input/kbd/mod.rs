pub mod keyboard;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct KeyCode(pub u8);

impl KeyCode {
    #[inline(always)]
    pub const fn new(row: u8, col: u8) -> Self {
        Self(((row & 0b111) << 5) | (col & 0b11111))
    }

    #[inline(always)]
    pub const fn row(self) -> u8 {
        self.0 >> 5
    }

    #[inline(always)]
    pub const fn col(self) -> u8 {
        self.0 & 0b11111
    }

    #[inline(always)]
    pub fn is_letter(self) -> bool {
        (KEY_FLAGS[self.0 as usize] & FLAG_LETTER) != 0
    }

    #[inline(always)]
    pub fn is_modifier(self) -> bool {
        (KEY_FLAGS[self.0 as usize] & FLAG_MODIFIER) != 0
    }

    #[inline(always)]
    pub fn is_numpad(self) -> bool {
        (KEY_FLAGS[self.0 as usize] & FLAG_NUMPAD) != 0
    }

    #[inline(always)]
    pub fn is_lock(self) -> bool {
        (KEY_FLAGS[self.0 as usize] & FLAG_LOCK) != 0
    }

    pub const ESCAPE: Self        = Self::new(0, 0);
    pub const F1: Self            = Self::new(0, 1);
    pub const F2: Self            = Self::new(0, 2);
    pub const F3: Self            = Self::new(0, 3);
    pub const F4: Self            = Self::new(0, 4);
    pub const F5: Self            = Self::new(0, 5);
    pub const F6: Self            = Self::new(0, 6);
    pub const F7: Self            = Self::new(0, 7);
    pub const F8: Self            = Self::new(0, 8);
    pub const F9: Self            = Self::new(0, 9);
    pub const F10: Self           = Self::new(0, 10);
    pub const F11: Self           = Self::new(0, 11);
    pub const F12: Self           = Self::new(0, 12);
    pub const PRINT_SCREEN: Self  = Self::new(0, 13);
    pub const SCROLL_LOCK: Self   = Self::new(0, 14);
    pub const PAUSE: Self         = Self::new(0, 15);

    pub const GRAVE: Self         = Self::new(1, 0);
    pub const KEY_1: Self         = Self::new(1, 1);
    pub const KEY_2: Self         = Self::new(1, 2);
    pub const KEY_3: Self         = Self::new(1, 3);
    pub const KEY_4: Self         = Self::new(1, 4);
    pub const KEY_5: Self         = Self::new(1, 5);
    pub const KEY_6: Self         = Self::new(1, 6);
    pub const KEY_7: Self         = Self::new(1, 7);
    pub const KEY_8: Self         = Self::new(1, 8);
    pub const KEY_9: Self         = Self::new(1, 9);
    pub const KEY_0: Self         = Self::new(1, 10);
    pub const MINUS: Self         = Self::new(1, 11);
    pub const EQUALS: Self        = Self::new(1, 12);
    pub const BACKSPACE: Self     = Self::new(1, 13);
    pub const INSERT: Self        = Self::new(1, 14);
    pub const HOME: Self          = Self::new(1, 15);
    pub const PAGE_UP: Self       = Self::new(1, 16);
    pub const NUMPAD_LOCK: Self   = Self::new(1, 17);
    pub const NUMPAD_DIVIDE: Self = Self::new(1, 18);
    pub const NUMPAD_MULTI: Self  = Self::new(1, 19);
    pub const NUMPAD_SUB: Self    = Self::new(1, 20);

    pub const TAB: Self           = Self::new(2, 0);
    pub const Q: Self             = Self::new(2, 1);
    pub const W: Self             = Self::new(2, 2);
    pub const E: Self             = Self::new(2, 3);
    pub const R: Self             = Self::new(2, 4);
    pub const T: Self             = Self::new(2, 5);
    pub const Y: Self             = Self::new(2, 6);
    pub const U: Self             = Self::new(2, 7);
    pub const I: Self             = Self::new(2, 8);
    pub const O: Self             = Self::new(2, 9);
    pub const P: Self             = Self::new(2, 10);
    pub const LEFT_BRACKET: Self  = Self::new(2, 11);
    pub const RIGHT_BRACKET: Self = Self::new(2, 12);
    pub const BACKSLASH: Self     = Self::new(2, 13);
    pub const DELETE: Self        = Self::new(2, 14);
    pub const END: Self           = Self::new(2, 15);
    pub const PAGE_DOWN: Self     = Self::new(2, 16);
    pub const NUMPAD_7: Self      = Self::new(2, 17);
    pub const NUMPAD_8: Self      = Self::new(2, 18);
    pub const NUMPAD_9: Self      = Self::new(2, 19);
    pub const NUMPAD_ADD: Self    = Self::new(2, 20);

    pub const CAPS_LOCK: Self     = Self::new(3, 0);
    pub const A: Self             = Self::new(3, 1);
    pub const S: Self             = Self::new(3, 2);
    pub const D: Self             = Self::new(3, 3);
    pub const F: Self             = Self::new(3, 4);
    pub const G: Self             = Self::new(3, 5);
    pub const H: Self             = Self::new(3, 6);
    pub const J: Self             = Self::new(3, 7);
    pub const K: Self             = Self::new(3, 8);
    pub const L: Self             = Self::new(3, 9);
    pub const SEMICOLON: Self     = Self::new(3, 10);
    pub const APOSTROPHE: Self    = Self::new(3, 11);
    pub const ENTER: Self         = Self::new(3, 12);
    pub const NUMPAD_4: Self      = Self::new(3, 17);
    pub const NUMPAD_5: Self      = Self::new(3, 18);
    pub const NUMPAD_6: Self      = Self::new(3, 19);

    pub const LEFT_SHIFT: Self    = Self::new(4, 0);
    pub const Z: Self             = Self::new(4, 1);
    pub const X: Self             = Self::new(4, 2);
    pub const C: Self             = Self::new(4, 3);
    pub const V: Self             = Self::new(4, 4);
    pub const B: Self             = Self::new(4, 5);
    pub const N: Self             = Self::new(4, 6);
    pub const M: Self             = Self::new(4, 7);
    pub const COMMA: Self         = Self::new(4, 8);
    pub const DOT: Self        = Self::new(4, 9);
    pub const SLASH: Self         = Self::new(4, 10);
    pub const RIGHT_SHIFT: Self   = Self::new(4, 11);
    pub const ARROW_UP: Self      = Self::new(4, 15);
    pub const NUMPAD_1: Self      = Self::new(4, 17);
    pub const NUMPAD_2: Self      = Self::new(4, 18);
    pub const NUMPAD_3: Self      = Self::new(4, 19);
    pub const NUMPAD_ENTER: Self  = Self::new(4, 20);

    pub const LEFT_CONTROL: Self  = Self::new(5, 0);
    pub const LEFT_SUPER: Self     = Self::new(5, 1);
    pub const LEFT_ALT: Self      = Self::new(5, 2);
    pub const SPACE: Self         = Self::new(5, 3);
    pub const RIGHT_ALT: Self     = Self::new(5, 4);
    pub const RIGHT_SUPER: Self    = Self::new(5, 5);
    pub const MENU: Self          = Self::new(5, 6);
    pub const RIGHT_CONTROL: Self = Self::new(5, 7);
    pub const ARROW_LEFT: Self    = Self::new(5, 14);
    pub const ARROW_DOWN: Self    = Self::new(5, 15);
    pub const ARROW_RIGHT: Self   = Self::new(5, 16);
    pub const NUMPAD_0: Self      = Self::new(5, 17);
    pub const NUMPAD_DECIMAL: Self= Self::new(5, 18);
}


/// Char lookup table (shift not pressed)
pub const KEYCODE_TO_CHAR_UNSHIFTED: [char; 256] = {
    let mut lut = ['\0'; 256];

    lut[KeyCode::GRAVE.0 as usize] = '`';
    lut[KeyCode::KEY_1.0 as usize] = '1';
    lut[KeyCode::KEY_2.0 as usize] = '2';
    lut[KeyCode::KEY_3.0 as usize] = '3';
    lut[KeyCode::KEY_4.0 as usize] = '4';
    lut[KeyCode::KEY_5.0 as usize] = '5';
    lut[KeyCode::KEY_6.0 as usize] = '6';
    lut[KeyCode::KEY_7.0 as usize] = '7';
    lut[KeyCode::KEY_8.0 as usize] = '8';
    lut[KeyCode::KEY_9.0 as usize] = '9';
    lut[KeyCode::KEY_0.0 as usize] = '0';
    lut[KeyCode::MINUS.0 as usize] = '-';
    lut[KeyCode::EQUALS.0 as usize] = '=';

    lut[KeyCode::TAB.0 as usize] = '\t';
    lut[KeyCode::Q.0 as usize] = 'q';
    lut[KeyCode::W.0 as usize] = 'w';
    lut[KeyCode::E.0 as usize] = 'e';
    lut[KeyCode::R.0 as usize] = 'r';
    lut[KeyCode::T.0 as usize] = 't';
    lut[KeyCode::Y.0 as usize] = 'y';
    lut[KeyCode::U.0 as usize] = 'u';
    lut[KeyCode::I.0 as usize] = 'i';
    lut[KeyCode::O.0 as usize] = 'o';
    lut[KeyCode::P.0 as usize] = 'p';
    lut[KeyCode::LEFT_BRACKET.0 as usize] = '[';
    lut[KeyCode::RIGHT_BRACKET.0 as usize] = ']';
    lut[KeyCode::BACKSLASH.0 as usize] = '\\';

    lut[KeyCode::A.0 as usize] = 'a';
    lut[KeyCode::S.0 as usize] = 's';
    lut[KeyCode::D.0 as usize] = 'd';
    lut[KeyCode::F.0 as usize] = 'f';
    lut[KeyCode::G.0 as usize] = 'g';
    lut[KeyCode::H.0 as usize] = 'h';
    lut[KeyCode::J.0 as usize] = 'j';
    lut[KeyCode::K.0 as usize] = 'k';
    lut[KeyCode::L.0 as usize] = 'l';
    lut[KeyCode::SEMICOLON.0 as usize] = ';';
    lut[KeyCode::APOSTROPHE.0 as usize] = '\'';
    lut[KeyCode::ENTER.0 as usize] = '\n';

    lut[KeyCode::Z.0 as usize] = 'z';
    lut[KeyCode::X.0 as usize] = 'x';
    lut[KeyCode::C.0 as usize] = 'c';
    lut[KeyCode::V.0 as usize] = 'v';
    lut[KeyCode::B.0 as usize] = 'b';
    lut[KeyCode::N.0 as usize] = 'n';
    lut[KeyCode::M.0 as usize] = 'm';
    lut[KeyCode::COMMA.0 as usize] = ',';
    lut[KeyCode::DOT.0 as usize] = '.';
    lut[KeyCode::SLASH.0 as usize] = '/';

    lut[KeyCode::SPACE.0 as usize] = ' ';

    lut[KeyCode::NUMPAD_DIVIDE.0 as usize] = '/';
    lut[KeyCode::NUMPAD_MULTI.0 as usize]  = '*';
    lut[KeyCode::NUMPAD_SUB.0 as usize]    = '-';
    lut[KeyCode::NUMPAD_ADD.0 as usize]    = '+';
    lut[KeyCode::NUMPAD_ENTER.0 as usize]  = '\n';
    lut[KeyCode::NUMPAD_1.0 as usize]      = '1';
    lut[KeyCode::NUMPAD_2.0 as usize]      = '2';
    lut[KeyCode::NUMPAD_3.0 as usize]      = '3';
    lut[KeyCode::NUMPAD_4.0 as usize]      = '4';
    lut[KeyCode::NUMPAD_5.0 as usize]      = '5';
    lut[KeyCode::NUMPAD_6.0 as usize]      = '6';
    lut[KeyCode::NUMPAD_7.0 as usize]      = '7';
    lut[KeyCode::NUMPAD_8.0 as usize]      = '8';
    lut[KeyCode::NUMPAD_9.0 as usize]      = '9';
    lut[KeyCode::NUMPAD_0.0 as usize]      = '0';
    lut[KeyCode::NUMPAD_DECIMAL.0 as usize]= '.';

    lut
};

/// Char lookup table (shift pressed)
pub const KEYCODE_TO_CHAR_SHIFTED: [char; 256] = {
    let mut lut = ['\0'; 256];

    lut[KeyCode::GRAVE.0 as usize] = '~';
    lut[KeyCode::KEY_1.0 as usize] = '!';
    lut[KeyCode::KEY_2.0 as usize] = '@';
    lut[KeyCode::KEY_3.0 as usize] = '#';
    lut[KeyCode::KEY_4.0 as usize] = '$';
    lut[KeyCode::KEY_5.0 as usize] = '%';
    lut[KeyCode::KEY_6.0 as usize] = '^';
    lut[KeyCode::KEY_7.0 as usize] = '&';
    lut[KeyCode::KEY_8.0 as usize] = '*';
    lut[KeyCode::KEY_9.0 as usize] = '(';
    lut[KeyCode::KEY_0.0 as usize] = ')';
    lut[KeyCode::MINUS.0 as usize] = '_';
    lut[KeyCode::EQUALS.0 as usize] = '+';

    lut[KeyCode::TAB.0 as usize] = '\t';
    lut[KeyCode::Q.0 as usize] = 'Q';
    lut[KeyCode::W.0 as usize] = 'W';
    lut[KeyCode::E.0 as usize] = 'E';
    lut[KeyCode::R.0 as usize] = 'R';
    lut[KeyCode::T.0 as usize] = 'T';
    lut[KeyCode::Y.0 as usize] = 'Y';
    lut[KeyCode::U.0 as usize] = 'U';
    lut[KeyCode::I.0 as usize] = 'I';
    lut[KeyCode::O.0 as usize] = 'O';
    lut[KeyCode::P.0 as usize] = 'P';
    lut[KeyCode::LEFT_BRACKET.0 as usize] = '{';
    lut[KeyCode::RIGHT_BRACKET.0 as usize] = '}';
    lut[KeyCode::BACKSLASH.0 as usize] = '|';

    lut[KeyCode::A.0 as usize] = 'A';
    lut[KeyCode::S.0 as usize] = 'S';
    lut[KeyCode::D.0 as usize] = 'D';
    lut[KeyCode::F.0 as usize] = 'F';
    lut[KeyCode::G.0 as usize] = 'G';
    lut[KeyCode::H.0 as usize] = 'H';
    lut[KeyCode::J.0 as usize] = 'J';
    lut[KeyCode::K.0 as usize] = 'K';
    lut[KeyCode::L.0 as usize] = 'L';
    lut[KeyCode::SEMICOLON.0 as usize] = ':';
    lut[KeyCode::APOSTROPHE.0 as usize] = '"';
    lut[KeyCode::ENTER.0 as usize] = '\n';

    lut[KeyCode::Z.0 as usize] = 'Z';
    lut[KeyCode::X.0 as usize] = 'X';
    lut[KeyCode::C.0 as usize] = 'C';
    lut[KeyCode::V.0 as usize] = 'V';
    lut[KeyCode::B.0 as usize] = 'B';
    lut[KeyCode::N.0 as usize] = 'N';
    lut[KeyCode::M.0 as usize] = 'M';
    lut[KeyCode::COMMA.0 as usize] = '<';
    lut[KeyCode::DOT.0 as usize] = '>';
    lut[KeyCode::SLASH.0 as usize] = '?';

    lut[KeyCode::SPACE.0 as usize] = ' ';

    lut[KeyCode::NUMPAD_DIVIDE.0 as usize] = '/';
    lut[KeyCode::NUMPAD_MULTI.0 as usize]  = '*';
    lut[KeyCode::NUMPAD_SUB.0 as usize]    = '-';
    lut[KeyCode::NUMPAD_ADD.0 as usize]    = '+';
    lut[KeyCode::NUMPAD_ENTER.0 as usize]  = '\n';
    lut[KeyCode::NUMPAD_1.0 as usize]      = '1';
    lut[KeyCode::NUMPAD_2.0 as usize]      = '2';
    lut[KeyCode::NUMPAD_3.0 as usize]      = '3';
    lut[KeyCode::NUMPAD_4.0 as usize]      = '4';
    lut[KeyCode::NUMPAD_5.0 as usize]      = '5';
    lut[KeyCode::NUMPAD_6.0 as usize]      = '6';
    lut[KeyCode::NUMPAD_7.0 as usize]      = '7';
    lut[KeyCode::NUMPAD_8.0 as usize]      = '8';
    lut[KeyCode::NUMPAD_9.0 as usize]      = '9';
    lut[KeyCode::NUMPAD_0.0 as usize]      = '0';
    lut[KeyCode::NUMPAD_DECIMAL.0 as usize]= '.';

    lut
};

#[inline(always)]
pub fn keycode_to_char(key: KeyCode, is_shift_pressed: bool, is_caps_lock_pressed: bool) -> Option<char> {
    let ch = if !is_caps_lock_pressed && !is_shift_pressed {
        //nothing pressed - everything small
        KEYCODE_TO_CHAR_UNSHIFTED[key.0 as usize]
    } else if is_caps_lock_pressed && !is_shift_pressed {
        //caps lock only - if letter big, if not small
        if key.is_letter() {
            KEYCODE_TO_CHAR_SHIFTED[key.0 as usize]
        } else {
            KEYCODE_TO_CHAR_UNSHIFTED[key.0 as usize]
        }
    } else if !is_caps_lock_pressed && is_shift_pressed {
        //shift only - everything big
        KEYCODE_TO_CHAR_SHIFTED[key.0 as usize]
    } else {
        //caps lock and shift - if letter small, if not big
        if key.is_letter() {
            KEYCODE_TO_CHAR_UNSHIFTED[key.0 as usize]
        } else {
            KEYCODE_TO_CHAR_SHIFTED[key.0 as usize]
        }
    };

    if ch == '\0' {
        None
    } else {
        Some(ch)
    }
}


pub const FLAG_NONE: u8     = 0;
pub const FLAG_LETTER: u8   = 1 << 0;
pub const FLAG_MODIFIER: u8 = 1 << 1;
pub const FLAG_NUMPAD: u8   = 1 << 2;
pub const FLAG_LOCK: u8     = 1 << 3;


pub const KEY_FLAGS: [u8; 256] = {
    let mut flags = [FLAG_NONE; 256];

    const fn set_flag(mut arr: [u8; 256], key: KeyCode, flag: u8) -> [u8; 256] {
        arr[key.0 as usize] |= flag;
        arr
    }

    flags = set_flag(flags, KeyCode::A, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::B, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::C, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::D, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::E, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::F, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::G, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::H, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::I, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::J, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::K, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::L, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::M, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::N, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::O, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::P, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::Q, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::R, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::S, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::T, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::U, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::V, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::W, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::X, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::Y, FLAG_LETTER);
    flags = set_flag(flags, KeyCode::Z, FLAG_LETTER);

    flags = set_flag(flags, KeyCode::LEFT_SHIFT, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::RIGHT_SHIFT, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::LEFT_CONTROL, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::RIGHT_CONTROL, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::LEFT_ALT, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::RIGHT_ALT, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::LEFT_SUPER, FLAG_MODIFIER);
    flags = set_flag(flags, KeyCode::RIGHT_SUPER, FLAG_MODIFIER);

    flags = set_flag(flags, KeyCode::CAPS_LOCK, FLAG_LOCK);
    flags = set_flag(flags, KeyCode::SCROLL_LOCK, FLAG_LOCK);

    flags = set_flag(flags, KeyCode::NUMPAD_LOCK, FLAG_LOCK | FLAG_NUMPAD);

    flags = set_flag(flags, KeyCode::NUMPAD_DIVIDE, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_MULTI, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_SUB, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_ADD, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_ENTER, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_1, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_2, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_3, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_4, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_5, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_6, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_7, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_8, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_9, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_0, FLAG_NUMPAD);
    flags = set_flag(flags, KeyCode::NUMPAD_DECIMAL, FLAG_NUMPAD);

    flags
};