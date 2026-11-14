pub mod usb_descriptors;
pub mod irq_mutex;
pub mod usb_transfers;
pub mod usb_core;
pub mod usb_hub;
pub mod usb_requests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum UsbSpeed {
    Unknown = 0,
    /// Full-speed (12 Mb/s)
    Full = 1,

    /// Low-speed (1.5 Mb/s)
    Low = 2,

    /// High-speed (480 Mb/s)
    High = 3,

    /// SuperSpeed (5 Gb/s)
    SuperSpeed = 4,

    /// SuperSpeedPlus (10 Gb/s or more)
    SuperSpeedPlus = 5,
}

impl UsbSpeed {
    pub fn from_xhci_port_speed(raw: u8) -> Self {
        match raw {
            1 => UsbSpeed::Full,
            2 => UsbSpeed::Low,
            3 => UsbSpeed::High,
            4 => UsbSpeed::SuperSpeed,
            5 => UsbSpeed::SuperSpeedPlus,
            _ => UsbSpeed::Unknown,
        }
    }

    pub fn to_xhci_slot_context_speed(self) -> u8 {
        self as u8
    }

    pub fn is_superspeed_or_higher(&self) -> bool {
        matches!(self, UsbSpeed::SuperSpeed | UsbSpeed::SuperSpeedPlus)
    }

    pub fn is_usb2_or_lower(&self) -> bool {
        matches!(self, UsbSpeed::Low | UsbSpeed::Full | UsbSpeed::High)
    }
}