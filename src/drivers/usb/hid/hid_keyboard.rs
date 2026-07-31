/*
 * Created by Antoni Kuczyński
 * 29/07/2026
 */
use alloc::sync::Arc;
use crate::drivers::input::kbd::keyboard::{handle_key_event, KeyAction, KeyEvent};
use crate::drivers::input::kbd::KeyCode;
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::DeviceToHost;
use crate::drivers::usb::hid::hid_core::{on_interrupt_in, HidDriver, RawHidReport};
use crate::kprintln;
use crate::memory::dma::{dma_alloc_zeroed};

//TODO: typematic support
pub struct HidKeyboard {
    device: Arc<UsbDevice>,
    in_endpoint_address: u8,
    pending_request: Option<Arc<IrqMutex<UsbTransferRequest>>>,
    previous_key_state: [u8; 8],
}

impl HidKeyboard {
    pub fn new(device: Arc<UsbDevice>, in_endpoint_address: u8) -> Self {
        Self {
            device,
            in_endpoint_address,
            pending_request: None,
            previous_key_state: [0; 8]
        }
    }
}

impl HidDriver for HidKeyboard {
    fn handle_hid_report(&mut self, report: RawHidReport) {
        if report.data.len() == 0 {
            kprintln!(Debug, "[HID KBD] Received HID report size=0, skipping.");
            return;
        }

        let new_keys = report.data;
        let old_keys = self.previous_key_state;
        let changed_modifiers = new_keys[0] ^ old_keys[0];

        if changed_modifiers != 0 {
            for &(mask, key_code) in MODIFIER_MAPPINGS.iter() {
                if (changed_modifiers & mask) != 0 {
                    let action = if (new_keys[0] & mask) != 0 {
                        KeyAction::Pressed
                    } else {
                        KeyAction::Released
                    };

                    let key_event = KeyEvent::new(key_code, action, true);
                    handle_key_event(key_event);
                }
            }
        }

        for key in new_keys {
            if *key != 0 && !old_keys.contains(key) {
                let key_code = match hid_to_keycode(*key) {
                    None => continue,
                    Some(x) => x
                };
                let key_event = KeyEvent::new(
                    key_code,
                    KeyAction::Pressed,
                    false
                );
                handle_key_event(key_event);
            }
        }

        for key in old_keys {
            if key != 0 && !new_keys.contains(&key) {
                let key_code = match hid_to_keycode(key) {
                    None => continue,
                    Some(x) => x
                };
                let key_event = KeyEvent::new(
                    key_code,
                    KeyAction::Released,
                    false
                );
                handle_key_event(key_event);
            }
        }

        self.previous_key_state = <[u8; 8]>::try_from(new_keys).expect("invalid hid packet");
    }

    fn start_listening(&mut self) {
        let device = self.device.clone();

        let dma = {
            dma_alloc_zeroed(8, 8).expect("Failed to allocate keyboard boot buffer.")
        };


        let request = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: device.clone(),
            endpoint_address: self.in_endpoint_address,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Interrupt,
            setup_packet: None,
            dma_buffer: Some(dma),
            data_buffer_length: 8,
            status: UsbTransferStatus::Pending,
            bytes_transferred: 0,
            completion_callback: Some(on_interrupt_in),
        }));

        self.pending_request = Some(request.clone());

        if let Some(controller) = device.host_controller.upgrade() {
            let _ = controller.submit_request(request);
        }
    }
}

pub const MODIFIER_MAPPINGS: [(u8, KeyCode); 8] = [
    (0x01, KeyCode::LEFT_CONTROL),
    (0x02, KeyCode::LEFT_SHIFT),
    (0x04, KeyCode::LEFT_ALT),
    (0x08, KeyCode::LEFT_SUPER),
    (0x10, KeyCode::RIGHT_CONTROL),
    (0x20, KeyCode::RIGHT_SHIFT),
    (0x40, KeyCode::RIGHT_ALT),
    (0x80, KeyCode::RIGHT_SUPER),
];

pub mod usb_hid {
    pub const KEY_NONE: u8 = 0x00;
    pub const KEY_ERR_OVF: u8 = 0x01;

    pub const KEY_A: u8 = 0x04;
    pub const KEY_B: u8 = 0x05;
    pub const KEY_C: u8 = 0x06;
    pub const KEY_D: u8 = 0x07;
    pub const KEY_E: u8 = 0x08;
    pub const KEY_F: u8 = 0x09;
    pub const KEY_G: u8 = 0x0a;
    pub const KEY_H: u8 = 0x0b;
    pub const KEY_I: u8 = 0x0c;
    pub const KEY_J: u8 = 0x0d;
    pub const KEY_K: u8 = 0x0e;
    pub const KEY_L: u8 = 0x0f;
    pub const KEY_M: u8 = 0x10;
    pub const KEY_N: u8 = 0x11;
    pub const KEY_O: u8 = 0x12;
    pub const KEY_P: u8 = 0x13;
    pub const KEY_Q: u8 = 0x14;
    pub const KEY_R: u8 = 0x15;
    pub const KEY_S: u8 = 0x16;
    pub const KEY_T: u8 = 0x17;
    pub const KEY_U: u8 = 0x18;
    pub const KEY_V: u8 = 0x19;
    pub const KEY_W: u8 = 0x1a;
    pub const KEY_X: u8 = 0x1b;
    pub const KEY_Y: u8 = 0x1c;
    pub const KEY_Z: u8 = 0x1d;

    pub const KEY_1: u8 = 0x1e;
    pub const KEY_2: u8 = 0x1f;
    pub const KEY_3: u8 = 0x20;
    pub const KEY_4: u8 = 0x21;
    pub const KEY_5: u8 = 0x22;
    pub const KEY_6: u8 = 0x23;
    pub const KEY_7: u8 = 0x24;
    pub const KEY_8: u8 = 0x25;
    pub const KEY_9: u8 = 0x26;
    pub const KEY_0: u8 = 0x27;

    pub const KEY_ENTER: u8 = 0x28;
    pub const KEY_ESC: u8 = 0x29;
    pub const KEY_BACKSPACE: u8 = 0x2a;
    pub const KEY_TAB: u8 = 0x2b;
    pub const KEY_SPACE: u8 = 0x2c;
    pub const KEY_MINUS: u8 = 0x2d;
    pub const KEY_EQUAL: u8 = 0x2e;
    pub const KEY_LEFTBRACE: u8 = 0x2f;
    pub const KEY_RIGHTBRACE: u8 = 0x30;
    pub const KEY_BACKSLASH: u8 = 0x31;
    pub const KEY_SEMICOLON: u8 = 0x33;
    pub const KEY_APOSTROPHE: u8 = 0x34;
    pub const KEY_GRAVE: u8 = 0x35;
    pub const KEY_COMMA: u8 = 0x36;
    pub const KEY_DOT: u8 = 0x37;
    pub const KEY_SLASH: u8 = 0x38;
    pub const KEY_CAPSLOCK: u8 = 0x39;

    pub const KEY_F1: u8 = 0x3a;
    pub const KEY_F2: u8 = 0x3b;
    pub const KEY_F3: u8 = 0x3c;
    pub const KEY_F4: u8 = 0x3d;
    pub const KEY_F5: u8 = 0x3e;
    pub const KEY_F6: u8 = 0x3f;
    pub const KEY_F7: u8 = 0x40;
    pub const KEY_F8: u8 = 0x41;
    pub const KEY_F9: u8 = 0x42;
    pub const KEY_F10: u8 = 0x43;
    pub const KEY_F11: u8 = 0x44;
    pub const KEY_F12: u8 = 0x45;

    pub const KEY_RIGHT: u8 = 0x4f;
    pub const KEY_LEFT: u8 = 0x50;
    pub const KEY_DOWN: u8 = 0x51;
    pub const KEY_UP: u8 = 0x52;

    pub const KEY_NUMLOCK: u8 = 0x53;
    pub const KEY_KPSLASH: u8 = 0x54;
    pub const KEY_KPASTERISK: u8 = 0x55;
    pub const KEY_KPMINUS: u8 = 0x56;
    pub const KEY_KPPLUS: u8 = 0x57;
    pub const KEY_KPENTER: u8 = 0x58;
    pub const KEY_KP1: u8 = 0x59;
    pub const KEY_KP2: u8 = 0x5a;
    pub const KEY_KP3: u8 = 0x5b;
    pub const KEY_KP4: u8 = 0x5c;
    pub const KEY_KP5: u8 = 0x5d;
    pub const KEY_KP6: u8 = 0x5e;
    pub const KEY_KP7: u8 = 0x5f;
    pub const KEY_KP8: u8 = 0x60;
    pub const KEY_KP9: u8 = 0x61;
    pub const KEY_KP0: u8 = 0x62;
    pub const KEY_KPDOT: u8 = 0x63;
}


pub const HID_TO_KEYCODE_LUT: [Option<KeyCode>; 256] = {
    let mut lut = [None; 256];

    use usb_hid::*;

    lut[KEY_A as usize] = Some(KeyCode::A);
    lut[KEY_B as usize] = Some(KeyCode::B);
    lut[KEY_C as usize] = Some(KeyCode::C);
    lut[KEY_D as usize] = Some(KeyCode::D);
    lut[KEY_E as usize] = Some(KeyCode::E);
    lut[KEY_F as usize] = Some(KeyCode::F);
    lut[KEY_G as usize] = Some(KeyCode::G);
    lut[KEY_H as usize] = Some(KeyCode::H);
    lut[KEY_I as usize] = Some(KeyCode::I);
    lut[KEY_J as usize] = Some(KeyCode::J);
    lut[KEY_K as usize] = Some(KeyCode::K);
    lut[KEY_L as usize] = Some(KeyCode::L);
    lut[KEY_M as usize] = Some(KeyCode::M);
    lut[KEY_N as usize] = Some(KeyCode::N);
    lut[KEY_O as usize] = Some(KeyCode::O);
    lut[KEY_P as usize] = Some(KeyCode::P);
    lut[KEY_Q as usize] = Some(KeyCode::Q);
    lut[KEY_R as usize] = Some(KeyCode::R);
    lut[KEY_S as usize] = Some(KeyCode::S);
    lut[KEY_T as usize] = Some(KeyCode::T);
    lut[KEY_U as usize] = Some(KeyCode::U);
    lut[KEY_V as usize] = Some(KeyCode::V);
    lut[KEY_W as usize] = Some(KeyCode::W);
    lut[KEY_X as usize] = Some(KeyCode::X);
    lut[KEY_Y as usize] = Some(KeyCode::Y);
    lut[KEY_Z as usize] = Some(KeyCode::Z);

    lut[KEY_1 as usize] = Some(KeyCode::KEY_1);
    lut[KEY_2 as usize] = Some(KeyCode::KEY_2);
    lut[KEY_3 as usize] = Some(KeyCode::KEY_3);
    lut[KEY_4 as usize] = Some(KeyCode::KEY_4);
    lut[KEY_5 as usize] = Some(KeyCode::KEY_5);
    lut[KEY_6 as usize] = Some(KeyCode::KEY_6);
    lut[KEY_7 as usize] = Some(KeyCode::KEY_7);
    lut[KEY_8 as usize] = Some(KeyCode::KEY_8);
    lut[KEY_9 as usize] = Some(KeyCode::KEY_9);
    lut[KEY_0 as usize] = Some(KeyCode::KEY_0);

    lut[KEY_ENTER as usize] = Some(KeyCode::ENTER);
    lut[KEY_ESC as usize] = Some(KeyCode::ESCAPE);
    lut[KEY_BACKSPACE as usize] = Some(KeyCode::BACKSPACE);
    lut[KEY_TAB as usize] = Some(KeyCode::TAB);
    lut[KEY_SPACE as usize] = Some(KeyCode::SPACE);
    lut[KEY_MINUS as usize] = Some(KeyCode::MINUS);
    lut[KEY_EQUAL as usize] = Some(KeyCode::EQUALS);
    lut[KEY_LEFTBRACE as usize] = Some(KeyCode::LEFT_BRACKET);
    lut[KEY_RIGHTBRACE as usize] = Some(KeyCode::RIGHT_BRACKET);
    lut[KEY_BACKSLASH as usize] = Some(KeyCode::BACKSLASH);
    lut[KEY_SEMICOLON as usize] = Some(KeyCode::SEMICOLON);
    lut[KEY_APOSTROPHE as usize] = Some(KeyCode::APOSTROPHE);
    lut[KEY_GRAVE as usize] = Some(KeyCode::GRAVE);
    lut[KEY_COMMA as usize] = Some(KeyCode::COMMA);
    lut[KEY_DOT as usize] = Some(KeyCode::DOT);
    lut[KEY_SLASH as usize] = Some(KeyCode::SLASH);
    lut[KEY_CAPSLOCK as usize] = Some(KeyCode::CAPS_LOCK);

    lut[KEY_F1 as usize] = Some(KeyCode::F1);
    lut[KEY_F2 as usize] = Some(KeyCode::F2);
    lut[KEY_F3 as usize] = Some(KeyCode::F3);
    lut[KEY_F4 as usize] = Some(KeyCode::F4);
    lut[KEY_F5 as usize] = Some(KeyCode::F5);
    lut[KEY_F6 as usize] = Some(KeyCode::F6);
    lut[KEY_F7 as usize] = Some(KeyCode::F7);
    lut[KEY_F8 as usize] = Some(KeyCode::F8);
    lut[KEY_F9 as usize] = Some(KeyCode::F9);
    lut[KEY_F10 as usize] = Some(KeyCode::F10);
    lut[KEY_F11 as usize] = Some(KeyCode::F11);
    lut[KEY_F12 as usize] = Some(KeyCode::F12);

    lut[KEY_RIGHT as usize] = Some(KeyCode::ARROW_RIGHT);
    lut[KEY_LEFT as usize] = Some(KeyCode::ARROW_LEFT);
    lut[KEY_DOWN as usize] = Some(KeyCode::ARROW_DOWN);
    lut[KEY_UP as usize] = Some(KeyCode::ARROW_UP);

    lut[KEY_NUMLOCK as usize] = Some(KeyCode::NUMPAD_LOCK);
    lut[KEY_KPSLASH as usize] = Some(KeyCode::NUMPAD_DIVIDE);
    lut[KEY_KPASTERISK as usize] = Some(KeyCode::NUMPAD_MULTI);
    lut[KEY_KPMINUS as usize] = Some(KeyCode::NUMPAD_SUB);
    lut[KEY_KPPLUS as usize] = Some(KeyCode::NUMPAD_ADD);
    lut[KEY_KPENTER as usize] = Some(KeyCode::NUMPAD_ENTER);

    lut[KEY_KP1 as usize] = Some(KeyCode::NUMPAD_1);
    lut[KEY_KP2 as usize] = Some(KeyCode::NUMPAD_2);
    lut[KEY_KP3 as usize] = Some(KeyCode::NUMPAD_3);
    lut[KEY_KP4 as usize] = Some(KeyCode::NUMPAD_4);
    lut[KEY_KP5 as usize] = Some(KeyCode::NUMPAD_5);
    lut[KEY_KP6 as usize] = Some(KeyCode::NUMPAD_6);
    lut[KEY_KP7 as usize] = Some(KeyCode::NUMPAD_7);
    lut[KEY_KP8 as usize] = Some(KeyCode::NUMPAD_8);
    lut[KEY_KP9 as usize] = Some(KeyCode::NUMPAD_9);
    lut[KEY_KP0 as usize] = Some(KeyCode::NUMPAD_0);
    lut[KEY_KPDOT as usize] = Some(KeyCode::NUMPAD_DECIMAL);

    lut
};

#[inline(always)]
pub fn hid_to_keycode(hid_code: u8) -> Option<KeyCode> {
    HID_TO_KEYCODE_LUT[hid_code as usize]
}