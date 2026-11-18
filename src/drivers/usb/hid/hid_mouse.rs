/*
 * Created by Antoni Kuczyński
 * 31/07/2026
 */
use alloc::sync::Arc;
use core::ptr::slice_from_raw_parts;
use crate::drivers::input::mouse::mouse::{handle_mouse_event, MouseButton, MouseButtonAction, MouseEvent};
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_requests::usb_request_interrupt_in;
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::hid::hid_core::{on_interrupt_in, HidDriver, RawHidReport};
use crate::{kprintln_hid_core, kprintln_hid_core_debug};
use crate::video::framebuffer::{fb_height, fb_width};

const CH9329_VENDOR_ID: u16 = 0x1a86;
const CH9329_PRODUCT_ID: u16 = 0xe129;
const CH9329_ABS_MAX: isize = 4096; //from HID descriptor

pub struct HidMouse {
    device: Arc<UsbDevice>,
    in_endpoint_address: u8,
    pending_request: Option<Arc<IrqMutex<UsbTransferRequest>>>,
    previous_button_state: u8,
    x_pos: isize,
    y_pos: isize
}


impl HidMouse {
    pub fn new(device: Arc<UsbDevice>, in_endpoint_address: u8) -> Self {
        Self {
            device,
            in_endpoint_address,
            pending_request: None,
            x_pos: 0,
            y_pos: 0,
            previous_button_state: 0
        }
    }
}

pub const MOUSE_BUTTONS_MAPPINGS: [(u8, MouseButton); 5] = [
    (0x01, MouseButton::Left),
    (0x02, MouseButton::Right),
    (0x04, MouseButton::Middle),
    (0x08, MouseButton::SideBack),
    (0x10, MouseButton::SideFront)
];

/*
        CH9329 absolute mouse report
        for now (but probably for forever) I'm hardcoding this here, so that my debugging tools work,
        without parsing the HID descriptor - there's no absolute mouse in boot protocol, and relative one sucks

        [0]: report type - absolute=0x02, relative=0x01
        [1]: unused, always zero
        [2]: X absolute pos [low byte]
        [3]: X absolute pos [high byte]
        [4]: Y absolute pos [low byte]
        [5]: Y absolute pos [high byte]
        [6]: unused, my CH9329 doesnt carry button presses or scroll in this mode
*/
fn handle_ch92329_absolute_quirk(data: &[u8]) {
    if data.len() < 5 {
        kprintln_hid_core_debug!( "Skipping invalid CH9329 absolute mouse data packet with length {} < 5.", data.len());
        return;
    }

    let x = u16::from_le_bytes([data[2], data[3]]);
    let y = u16::from_le_bytes([data[4], data[5]]);

    let px = x as isize * fb_width() as isize / CH9329_ABS_MAX;
    let py = y as isize * fb_height() as isize / CH9329_ABS_MAX;

    let mouse_event = MouseEvent::new_move_absolute(px, py);
    handle_mouse_event(mouse_event);
}


impl HidDriver for HidMouse {
    fn handle_hid_report(&mut self, report: RawHidReport) {
        let mut data = report.data;

        let Some(desc) = self.device.device_descriptor.get() else {
            kprintln_hid_core!(Error, "[HID MOUSE] Report from a device without its device descriptor.");
            return;
        };

        if data.len() < 3 {
            kprintln_hid_core_debug!( "Skipping invalid data packet with length {} < 3.", data.len());
            return;
        }

        //stupid CH9329 quirk, purely out of my laziness
        if desc.id_vendor() == CH9329_VENDOR_ID && desc.id_product() == CH9329_PRODUCT_ID {
            if data[0] == 0x01 {
                //relative mouse - remove the report type byte
                data = unsafe { &*slice_from_raw_parts(data.as_ptr().add(1), 4) };
            } else if data[0] == 0x02 {
                handle_ch92329_absolute_quirk(data);
                return;
            }
        }

        if data[1] != 0 || data[2] != 0 {
            let x_raw = (data[1] as i8) as isize;
            let y_raw = (data[2] as i8) as isize;

            let sign_x = if x_raw == 1 { -1 } else { 1 };
            let sign_y = if y_raw == 1 { -1 } else { 1 };

            let move_x = x_raw * sign_x;
            let move_y = y_raw * sign_y;

            let mouse_event = MouseEvent::new_move_relative(move_x, move_y);
            handle_mouse_event(mouse_event);
        }


        let new_buttons = data[0];
        let old_buttons = self.previous_button_state;
        let changed_buttons = new_buttons ^ old_buttons;

        if changed_buttons != 0 {
            for &(mask, mouse_button) in MOUSE_BUTTONS_MAPPINGS.iter() {
                if (changed_buttons & mask) != 0 {
                    let action = if (new_buttons & mask) != 0 {
                        MouseButtonAction::Pressed
                    } else {
                        MouseButtonAction::Released
                    };

                    let mouse_event = MouseEvent::new_click(mouse_button, action);
                    handle_mouse_event(mouse_event);
                }
            }
        }


        //well, this is sketchy but works on some mice.
        //the boot protocol officially doesn't support scroll data - it only sends 3 bytes (button status, x,y movement)
        //but some mice don't care about being in boot protocol and still send you their full
        //4 byte report with scroll data in it
        let mut scroll_delta = 0isize;
        if data.len() >= 4 {
            scroll_delta = (data[3] as i8) as isize;
        }

        if scroll_delta != 0 {
            let mouse_event = MouseEvent::new_scroll(scroll_delta);
            handle_mouse_event(mouse_event);
        }
    }

    fn start_listening(&mut self) {
        let device = self.device.clone();

        let Some(endpoint) = device
            .configuration_tree
            .get()
            .and_then(|tree| tree.find_endpoint(self.in_endpoint_address)) else {
            kprintln_hid_core!(Error, "[HID MOUSE] IN endpoint {:#x} not found in the configuration tree.", self.in_endpoint_address);
            return;
        };

        let max_packet_size = endpoint.w_max_packet_size as usize;

        kprintln_hid_core_debug!( "[HID MOUSE] Allocating buffer with size w_max_packet_size={}.", max_packet_size);

        let Some(request) = usb_request_interrupt_in(device.clone(), self.in_endpoint_address, max_packet_size, on_interrupt_in) else {
            kprintln_hid_core!(Error, "[HID MOUSE] Failed to allocate interrupt IN request.");
            return;
        };

        self.pending_request = Some(request.clone());

        if let Some(controller) = device.host_controller.upgrade() {
            let _ = controller.submit_request(request);
        }
    }
}

