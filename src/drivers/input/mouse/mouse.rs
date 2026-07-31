/*
 * Created by Antoni Kuczyński
 * 31/07/2026
 */
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use spin::Once;
use crate::drivers::input::mouse::{draw_cursor, CURSOR_BYTE_SIZE};
use crate::drivers::input::mouse::mouse::MouseButtonAction::{Pressed, Released};
use crate::kprintln;
use crate::video::framebuffer::{fb_height, fb_width, FRAMEBUFFER};

#[derive(Copy, Clone, PartialEq)]
pub enum MouseButtonAction {
    Pressed = 1,
    Released = 2,
}

#[derive(Copy, Clone, PartialEq, Ord, PartialOrd, Eq, Debug)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    SideBack = 3,
    SideFront = 4,
}

#[derive(Copy, Clone, PartialEq)]
pub struct MouseEvent {
    move_x: isize,
    move_y: isize,
    mouse_button: Option<MouseButton>,
    mouse_button_action: Option<MouseButtonAction>,
    scroll: isize
}

impl MouseEvent {
    pub fn new_click(mouse_button: MouseButton, mouse_button_action: MouseButtonAction) -> Self {
        Self {
            move_x: 0,
            move_y: 0,
            mouse_button: Some(mouse_button),
            mouse_button_action: Some(mouse_button_action),
            scroll: 0
        }
    }

    pub fn new_move(move_x : isize, move_y: isize) -> Self {
        Self {
            move_x,
            move_y,
            mouse_button: None,
            mouse_button_action: None,
            scroll: 0
        }
    }

    pub fn new_scroll(scroll: isize) -> Self {
        Self {
            move_x: 0,
            move_y: 0,
            mouse_button: None,
            mouse_button_action: None,
            scroll
        }
    }
}

pub struct GlobalMouse {
    pub pressed_buttons: BTreeMap<MouseButton, AtomicBool>,
    pub x_pos: AtomicIsize,
    pub y_pos: AtomicIsize,
}

impl GlobalMouse {
    pub fn init() {
        let mid_x = fb_width() / 2;
        let mid_y = fb_height() / 2;

        let mut pressed_buttons = BTreeMap::new();

        pressed_buttons.insert(MouseButton::Left, AtomicBool::new(false));
        pressed_buttons.insert(MouseButton::Right, AtomicBool::new(false));
        pressed_buttons.insert(MouseButton::Middle, AtomicBool::new(false));
        pressed_buttons.insert(MouseButton::SideBack, AtomicBool::new(false));
        pressed_buttons.insert(MouseButton::SideFront, AtomicBool::new(false));

        let mut previous_video_buffer: Vec<u8> = Vec::with_capacity(CURSOR_BYTE_SIZE);
        for i in 0..CURSOR_BYTE_SIZE {
            previous_video_buffer.push(0u8); //so that the length is not zero
        }

        let mouse = GlobalMouse {
            pressed_buttons,
            x_pos: AtomicIsize::new(mid_x as isize),
            y_pos: AtomicIsize::new(mid_y as isize),
        };

        GLOBAL_MOUSE.call_once(|| mouse);
    }
}

pub fn handle_mouse_event(event: MouseEvent) {
    let mouse = GLOBAL_MOUSE.get()
        .expect("global mouse not initialized");

    if let (Some(btn), Some(action)) = (event.mouse_button, event.mouse_button_action) {
        let btn_bool = mouse.pressed_buttons.get(&btn).unwrap();
        if action == Pressed {
            kprintln!("{:?} mouse button pressed.", btn);
            btn_bool.fetch_or(true, Ordering::SeqCst);
        } else if action == Released {
            btn_bool.fetch_and(false, Ordering::SeqCst);
        }
    }

    if event.move_x == 0 && event.move_y == 0 {
        return;
    }

    let prev_x = mouse.x_pos.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current_x| {
        let new_x = current_x + event.move_x;
        Some(new_x.clamp(0, fb_width() as isize - 1))
    }).unwrap();

    let prev_y = mouse.y_pos.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current_y| {
        let new_y = current_y + event.move_y;
        Some(new_y.clamp(0, fb_height() as isize - 1))
    }).unwrap();

    {
        let fb = FRAMEBUFFER.lock();
        // fb.as_mut().unwrap().plot_pixel(mouse.x_pos.load(Ordering::Relaxed) as usize, mouse.y_pos.load(Ordering::Relaxed) as usize, &FramebufferColor::from_rgb(255,0,0));

        draw_cursor(fb);
    }

    // kprintln!("({},{})", mouse.x_pos.load(Ordering::Relaxed), mouse.y_pos.load(Ordering::Relaxed));
}

pub static GLOBAL_MOUSE: Once<GlobalMouse> = Once::new();

