/*
 * Created by Antoni Kuczyński
 * 29/07/2026
 */
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::once::Once;
use crate::drivers::input::kbd::{keycode_to_char, KeyCode};
use crate::{kprint_raw};
use crate::drivers::input::kbd::keyboard::KeyAction::Pressed;

#[derive(Copy, Clone, PartialEq)]
pub enum KeyAction {
    Pressed = 1,
    Released = 2,
}

#[derive(Copy, Clone)]
pub struct KeyEvent {
    pub key_code: KeyCode,
    pub key_action: KeyAction,
    pub is_modifier: bool
}

impl KeyEvent {
    pub fn new(key_code: KeyCode, key_action: KeyAction, is_modifier: bool) -> KeyEvent {
        KeyEvent {
            key_code,
            key_action,
            is_modifier
        }
    }
}

pub struct GlobalKeyboard {
    active_modifiers: BTreeMap<KeyCode, AtomicBool>
}

impl GlobalKeyboard {
    pub fn init() {
        let mut active_modifiers = BTreeMap::new();

        active_modifiers.insert(KeyCode::CAPS_LOCK, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::LEFT_SHIFT, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::LEFT_CONTROL, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::LEFT_SUPER, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::LEFT_ALT, AtomicBool::new(false));

        active_modifiers.insert(KeyCode::RIGHT_ALT, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::RIGHT_SUPER, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::RIGHT_CONTROL, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::RIGHT_SHIFT, AtomicBool::new(false));

        active_modifiers.insert(KeyCode::NUMPAD_ENTER, AtomicBool::new(false));
        active_modifiers.insert(KeyCode::NUMPAD_LOCK, AtomicBool::new(false));

        let kbd = GlobalKeyboard {
            active_modifiers
        };

        GLOBAL_KEYBOARD.call_once(|| kbd);
    }
}

fn handle_key_press(key_event: KeyEvent, kbd: &GlobalKeyboard) {
    let mods = &kbd.active_modifiers;

    if key_event.key_code == KeyCode::CAPS_LOCK {
        if let Some(caps_atomic) = mods.get(&KeyCode::CAPS_LOCK) {
            caps_atomic.fetch_xor(true, Ordering::SeqCst);
        }
        return;
    }

    if key_event.is_modifier {
        if let Some(modifier_atomic) = mods.get(&key_event.key_code) {
            modifier_atomic.fetch_or(true, Ordering::SeqCst);
        }
        return;
    }


    let is_caps_on = mods.get(&KeyCode::CAPS_LOCK)
        .map(|a| a.load(Ordering::SeqCst))
        .unwrap_or(false);

    let is_left_shift_on = mods.get(&KeyCode::LEFT_SHIFT)
        .map(|a| a.load(Ordering::SeqCst))
        .unwrap_or(false);

    let is_right_shift_on = mods.get(&KeyCode::RIGHT_SHIFT)
        .map(|a| a.load(Ordering::SeqCst))
        .unwrap_or(false);

    let char = keycode_to_char(
        key_event.key_code,
        is_left_shift_on | is_right_shift_on,
        is_caps_on
    ).unwrap_or_else(|| '?');

    kprint_raw!("{}", char);
}

fn handle_key_release(key_event: KeyEvent, kbd: &GlobalKeyboard) {
    if key_event.is_modifier {
        if let Some(modifier_atomic) = kbd.active_modifiers.get(&key_event.key_code) {
            modifier_atomic.fetch_and(false, Ordering::SeqCst);
        }
        return;
    }
}

pub fn handle_key_event(key_event: KeyEvent) {
    let kbd = GLOBAL_KEYBOARD.get()
        .expect("Global keyboard struct not initialized!");
    if key_event.key_action == Pressed {
        handle_key_press(key_event, kbd);
    } else {
        handle_key_release(key_event, kbd);
    }
}

pub static GLOBAL_KEYBOARD: Once<GlobalKeyboard> = Once::new();
