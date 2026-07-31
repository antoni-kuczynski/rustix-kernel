/*
 * Created by Antoni Kuczyński
 * 31/07/2026
 */
use alloc::sync::Arc;
use crate::drivers::input::mouse::mouse::{handle_mouse_event, MouseButton, MouseButtonAction, MouseEvent};
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_transfers::{UsbDevice, UsbTransferRequest, UsbTransferStatus, UsbTransferType};
use crate::drivers::usb::core::usb_transfers::UsbTransferDirection::DeviceToHost;
use crate::drivers::usb::hid::hid_core::{on_interrupt_in, HidDriver, RawHidReport};
use crate::kprintln;
use crate::memory::dma::{dma_alloc_zeroed};

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

impl HidDriver for HidMouse {
    fn handle_hid_report(&mut self, report: RawHidReport) {
        let data = report.data;

        if data.len() < 3 {
            kprintln!(Debug, "Skipping invalid data packet with length {} < 3.", data.len());
            return;
        }

        if data[1] != 0 && data[2] != 0 {
            let x_raw = (data[1] as i8) as isize;
            let y_raw = (data[2] as i8) as isize;

            let sign_x = if x_raw & 0x80 == 1 { -1 } else { 1 };
            let sign_y = if y_raw & 0x80 == 1 { -1 } else { 1 };

            let move_x = x_raw * sign_x;
            let move_y = y_raw * sign_y;

            let mouse_event = MouseEvent::new_move(move_x, move_y);
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

        //event though, mice should only send 3 bytes in boot protocol, we allocate 4 byte buffer
        //because some mice like to send additional byte which is scroll data
        let dma = {
            dma_alloc_zeroed(4, 4).expect("Failed to allocate mouse boot buffer.")
        };

        let request = Arc::new(IrqMutex::new(UsbTransferRequest {
            target_device: device.clone(),
            endpoint_address: self.in_endpoint_address,
            transfer_direction: DeviceToHost,
            transfer_type: UsbTransferType::Interrupt,
            setup_packet: None,
            dma_buffer: Some(dma),
            data_buffer_length: 4,
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

