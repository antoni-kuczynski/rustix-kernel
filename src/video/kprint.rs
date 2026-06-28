/*
 * Created by Antoni Kuczyński
 * 23/06/2026
 */
use core::fmt;
use core::fmt::{Arguments, Write};
use spin::{Mutex};
use crate::video::framebuffer::{Framebuffer, FramebufferColor, FRAMEBUFFER};
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
            LogLevel::Error => "-  FAIL  - ",
            LogLevel::Warn  => "-  WARN  - ",
            LogLevel::Info  => "-  INFO  - ",
            LogLevel::Debug => "-  DBUG  - ",
        }
    }

    pub fn color(&self) -> FramebufferColor {
        match self {
            LogLevel::Error => { FramebufferColor::from_rgb(190,0,0) }
            LogLevel::Warn => { FramebufferColor::from_rgb(180,110,0) }
            LogLevel::Info => { FramebufferColor::from_rgb(164,168,178) }
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


    if success {
        fb.current_foreground = FramebufferColor::from_rgb(0,160,0);
        _print(format_args!("-  DONE  - "), fb, false);
    } else {
        fb.current_foreground = FramebufferColor::from_rgb(170,0,0);
        _print(format_args!("-  FAIL  - "), fb, false);
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

    _print(format_args!("-  FATL  - "), fb, false);
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

const EARLY_BUF_SIZE: usize = 256;

/// Stores early debugging messages, while the framebuffer hasn't been initialized yet.
struct EarlyKPrintBuffer {
    buf: UnsafeCell<[u8; EARLY_BUF_SIZE]>,
    bump_index: usize,
}

unsafe impl Sync for EarlyKPrintBuffer {}

impl Write for EarlyKPrintBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        for i in 0..bytes.len() {
            if self.bump_index + i >= EARLY_BUF_SIZE {
                self.bump_index = EARLY_BUF_SIZE;
                self.buf.get_mut()[self.bump_index - 1] = 0x0A; //new line for clarity
                return Ok(())
            }
            self.buf.get_mut()[self.bump_index + i] = bytes[i];
        }
        self.bump_index += bytes.len();
        Ok(())
    }
}

/// Used for storing the early kprint messages, before the proper framebuffer is initialized.
static BUF: Mutex<EarlyKPrintBuffer> = Mutex::new(
    EarlyKPrintBuffer {
        buf: UnsafeCell::new([0; EARLY_BUF_SIZE]),
        bump_index: 0,
    }
);

/*
colors:
0x0 - white
0x01 - green
0x02 - red
 */

pub fn early_text_buffer_init() {
    let mut lock = FRAMEBUFFER.lock();
    let fb_option = lock.as_mut();
    if fb_option.is_none() {
        panic!("Tried to print early buffer to uninitialized framebuffer.");
    }

    let fb = fb_option.unwrap();
    let mut early_buf = BUF.lock();
    let buf_end = early_buf.bump_index;
    let data = early_buf.buf.get_mut();
    for i in 0..buf_end {
        if data[i] == 0x0 {
            fb.current_foreground = FramebufferColor::from_rgb(255, 255, 255);
        } else if data[i] == 0x01 {
            fb.current_foreground = FramebufferColor::from_rgb(0,160,0);
        } else if data[i] == 0x02 {
            fb.current_foreground = FramebufferColor::from_rgb(170,0,0);
        } else {
            fb.put_char_at_cursor(data[i] as char);
        }

    }

}

#[doc(hidden)]
pub fn _kprintln_buf(args: Arguments) {
    let mut buf = BUF.lock();
    let args1 = format_args!("\x00{}\n", args);
    buf.write_fmt(args1).expect("write to early fb buffer failed");
}

#[doc(hidden)]
pub fn _kprint_ok_buf(args: Arguments) {
    let mut buf = BUF.lock();
    let args1 = format_args!("\x01-  DONE  -\x00 {}\n", args);
    buf.write_fmt(args1).expect("write to early fb buffer failed");
}

#[doc(hidden)]
pub fn _kprint_failed_buf(args: Arguments) {
    let mut buf = BUF.lock();
    let args1 = format_args!("\x02-  FAIL  -\x00 {}\n", args);
    buf.write_fmt(args1).expect("write to early fb buffer failed");
}

#[macro_export]
macro_rules! __kprintln_buf {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprintln_buf(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! __kprintln_ok_buf {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint_ok_buf(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! __kprintln_failed_buf {
    ($($arg:tt)*) => {
        $crate::video::kprint::_kprint_failed_buf(format_args!($($arg)*));
    };
}
