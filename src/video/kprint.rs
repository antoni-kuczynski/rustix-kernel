/*
 * Created by Antoni Kuczyński
 * 23/06/2026
 */
use core::fmt;
use core::fmt::{Arguments, Write};
use core::ptr::{null};
use spin::{Mutex, MutexGuard, Once};
use crate::video::console::{fb_get_background, fb_get_foreground, fb_set_background, fb_set_foreground};
use crate::video::framebuffer::{fb_swap_buffers, Framebuffer, FramebufferColor, FRAMEBUFFER};
use core::cell::UnsafeCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl Write for Framebuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.put_string_at_cursor(s);
        Ok(())
    }
}


fn _print(args: Arguments, fb: &mut Framebuffer, swap_buffers: bool) {
    fb.write_fmt(args).expect("kprint failed");


    if fb.is_double_buffered && swap_buffers {
        fb.swap_buffers();
    }
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "[ FAIL ] ",
            LogLevel::Warn  => "[ WARN ] ",
            LogLevel::Info  => "[ INFO ] ",
            LogLevel::Debug => "[ DBUG ] ",
        }
    }

    pub fn color(&self) -> FramebufferColor {
        match self {
            LogLevel::Error => { FramebufferColor::from_rgb(190,0,0) }
            LogLevel::Warn => { FramebufferColor::from_rgb(180,110,0) }
            LogLevel::Info => { FramebufferColor::from_rgb(170,170,170) }
            LogLevel::Debug => { FramebufferColor::from_rgb(170,170,170) }
        }
    }
}

#[doc(hidden)]
pub fn _kprint_status(success: bool, args: Arguments) {
    let mut lock = FRAMEBUFFER.lock();
    let fb = lock.as_mut().unwrap();

    let colors = (fb.current_foreground, fb.current_background);
    fb.current_background = FramebufferColor::from_rgb(0,0,0);


    //wow, what an original logging style!
    //totally doesn't look like some other famous kernel's one...
    if success {
        fb.current_foreground = FramebufferColor::from_rgb(0,160,0);
        _print(format_args!("[  OK  ] "), fb, false);
    } else {
        fb.current_foreground = FramebufferColor::from_rgb(170,0,0);
        _print(format_args!("[FAILED] "), fb, false);
    }

    //restore old colors
    fb.current_foreground = colors.0;
    fb.current_background = colors.1;

    _print(format_args!("{} {}", args, "\n"), fb, true);
}


#[doc(hidden)]
pub fn _kprint_panic(args: Arguments) {
    let mut lock = FRAMEBUFFER.lock();
    let fb = lock.as_mut().unwrap();

    let colors = (fb.current_foreground, fb.current_background);
    fb.current_background = FramebufferColor::from_rgb(0,0,0);
    fb.current_foreground = FramebufferColor::from_rgb(220,0,0);

    _print(format_args!("[KPANIC] "), fb, false);
    _print(format_args!("{} {}", args, "\n"), fb, true);

    fb.current_foreground = colors.0;
    fb.current_background = colors.1;
}

#[doc(hidden)]
pub fn _kprint(level: LogLevel, args: Arguments) {
    let mut lock = FRAMEBUFFER.lock();
    let fb = lock.as_mut().unwrap();
    let colors = (fb.current_foreground, fb.current_background);

    fb.current_background = FramebufferColor::from_rgb(0,0,0);
    fb.current_foreground = level.color();

    _print(format_args!("{}", level.as_str()), fb, false);

    fb.current_foreground = colors.0;
    fb.current_background = colors.1;

    _print(format_args!("{}", args), fb, true);
}

#[doc(hidden)]
pub fn _kprintln(level: LogLevel, args: Arguments) {
    let args1 = format_args!("{}\n", args);
    _kprint(level, args1);
}

#[macro_export]
macro_rules! kprint {
    ($level:ident, $($arg:tt)*) => {
        $crate::video::kprint::_kprint($crate::video::kprint::LogLevel::$level, format_args!($($arg)*));
    };
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint($crate::video::kprint::LogLevel::Info, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! kprintln {
    () => {
        $crate::video::kprint::_kprintln($crate::video::kprint::LogLevel::Info, format_args!(""));
    };
    ($level:ident, $($arg:tt)*) => {
        $crate::video::kprint::_kprintln($crate::video::kprint::LogLevel::$level, format_args!($($arg)*));
    };
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprintln($crate::video::kprint::LogLevel::Info, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! kprintln_ok {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint_status(true, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! kprintln_failed {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint_status(false, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! kprintln_panic {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint_panic(format_args!($($arg)*));
    };
}

//TODO: unpub this
/// Stores early debugging messages, while the framebuffer hasn't been initialized yet.
pub struct EarlyKPrintBuffer {
    pub buf: UnsafeCell<[u8; 1024]>,
    bump_index: usize,
}

unsafe impl Sync for EarlyKPrintBuffer {}

//This is going to be used only at init, with only BSP active, so its thread safe, no mutex needed
pub static BUF: Once<EarlyKPrintBuffer> = Once::new();


/// Used for storing the early kprint messages, before the proper framebuffer is initialized.
pub fn early_fb_buffer_init() {
    let buf = EarlyKPrintBuffer {
        buf: UnsafeCell::new([0; 1024]),
        bump_index: 0,
    };

    BUF.call_once(|| buf);
}
