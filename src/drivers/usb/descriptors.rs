/*
 * Created by Antoni Kuczyński
 * 19/07/2026
 */
use alloc::vec::Vec;
//Just some stupid boilerplate code :(
use crate::drivers::usb::UsbId;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct UsbDeviceDescriptor {
    b_length: u8,
    b_descriptor_type: u8,
    bcd_usb: u16,
    b_device_class: u8,
    b_device_subclass: u8,
    b_device_protocol: u8,
    b_max_packet_size0: u8,
    id_vendor: u16,
    id_product: u16,
    bcd_device: u16,
    i_manufacturer: u8,
    i_product: u8,
    i_serial_number: u8,
    b_num_configurations: u8,
}

impl UsbDeviceDescriptor {
    pub fn b_length(&self) -> u8 {
        self.b_length
    }

    pub fn b_descriptor_type(&self) -> u8 {
        self.b_descriptor_type
    }

    pub fn bcd_usb(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.bcd_usb)) }
    }

    pub fn b_device_class(&self) -> u8 {
        self.b_device_class
    }

    pub fn b_device_subclass(&self) -> u8 {
        self.b_device_subclass
    }

    pub fn b_device_protocol(&self) -> u8 {
        self.b_device_protocol
    }

    pub fn b_max_packet_size0(&self) -> u8 {
        self.b_max_packet_size0
    }

    pub fn id_vendor(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.id_vendor)) }
    }

    pub fn id_product(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.id_product)) }
    }

    pub fn bcd_device(&self) -> u16 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.bcd_device)) }
    }

    pub fn i_manufacturer(&self) -> u8 {
        self.i_manufacturer
    }

    pub fn i_product(&self) -> u8 {
        self.i_product
    }

    pub fn i_serial_number(&self) -> u8 {
        self.i_serial_number
    }

    pub fn b_num_configurations(&self) -> u8 {
        self.b_num_configurations
    }

    pub fn usb_id(&self) -> UsbId {
        let vendor = self.id_vendor;
        let product = self.id_product;

        UsbId {
            vendor, product
        }
    }

    pub fn print(&self) {
        kprintln!(Debug,
        "UsbDeviceDescriptor {{
            b_length: {},
            b_descriptor_type: {},
            bcd_usb: {:#06x},
            b_device_class: {:#04x},
            b_device_subclass: {:#04x},
            b_device_protocol: {:#04x},
            b_max_packet_size0: {},
            id_vendor: {:#011x},
            id_product: {:#06x},
            bcd_device: {:#06x},
            i_manufacturer: {},
            i_product: {},
            i_serial_number: {},
            b_num_configurations: {}
        Additional info:
            Vendor name: {},
            Product name: {}
    }}",
        self.b_length(),
        self.b_descriptor_type(),
        self.bcd_usb(),
        self.b_device_class(),
        self.b_device_subclass(),
        self.b_device_protocol(),
        self.b_max_packet_size0(),
        self.id_vendor(),
        self.id_product(),
        self.bcd_device(),
        self.i_manufacturer(),
        self.i_product(),
        self.i_serial_number(),
        self.b_num_configurations(),
        self.usb_id().vendor_name(),
        self.usb_id().product_name()
    );
    }
}


use core::{ptr, slice};
use crate::kprintln;

pub struct DescriptorType;

impl DescriptorType {
    pub const DEVICE: u8 = 1;
    pub const CONFIGURATION: u8 = 2;
    pub const STRING: u8 = 3;
    pub const INTERFACE: u8 = 4;
    pub const ENDPOINT: u8 = 5;
    pub const HID: u8 = 0x21;
}

#[derive(Debug, Clone)]
pub struct ConfigurationDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
    pub i_configuration: u8,
    pub bm_attributes: u8,
    pub b_max_power: u8,
}

impl ConfigurationDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            w_total_length: u16::from_le_bytes([data[2], data[3]]),
            b_num_interfaces: data[4],
            b_configuration_value: data[5],
            i_configuration: data[6],
            bm_attributes: data[7],
            b_max_power: data[8],
        })
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_interface_number: u8,
    pub b_alternate_setting: u8,
    pub b_num_endpoints: u8,
    pub b_interface_class: u8,
    pub b_interface_sub_class: u8,
    pub b_interface_protocol: u8,
    pub i_interface: u8,
}

impl InterfaceDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            b_interface_number: data[2],
            b_alternate_setting: data[3],
            b_num_endpoints: data[4],
            b_interface_class: data[5],
            b_interface_sub_class: data[6],
            b_interface_protocol: data[7],
            i_interface: data[8],
        })
    }
}

#[derive(Debug, Clone)]
pub struct EndpointDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_endpoint_address: u8,
    pub bm_attributes: u8,
    pub w_max_packet_size: u16,
    pub b_interval: u8,
}

impl EndpointDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 {
            return None;
        }
        Some(Self {
            b_length: data[0],
            b_descriptor_type: data[1],
            b_endpoint_address: data[2],
            bm_attributes: data[3],
            w_max_packet_size: u16::from_le_bytes([data[4], data[5]]),
            b_interval: data[6],
        })
    }
}

#[derive(Debug, Clone)]
pub struct UsbInterfaceTree {
    pub interface: InterfaceDescriptor,
    pub endpoints: Vec<EndpointDescriptor>,
    pub other_descriptors: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct UsbConfigurationTree {
    pub configuration: ConfigurationDescriptor,
    pub interfaces: Vec<UsbInterfaceTree>,
    pub other_descriptors: Vec<Vec<u8>>,
}

impl UsbConfigurationTree {
    pub unsafe fn from_ptr(ptr: *const u8, size: usize) -> Option<Self> {
        let data = slice::from_raw_parts(ptr, size);
        Self::parse(data)
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let mut config_tree: Option<UsbConfigurationTree> = None;
        let mut current_iface: Option<UsbInterfaceTree> = None;

        let mut offset = 0;

        while offset < data.len() {
            let b_length = data[offset] as usize;

            if b_length == 0 || offset + b_length > data.len() {
                break;
            }

            let b_type = data[offset + 1];
            let chunk = &data[offset..offset + b_length];

            match b_type {
                0x02 => {
                    if let Some(desc) = ConfigurationDescriptor::parse(chunk) {
                        config_tree = Some(UsbConfigurationTree {
                            configuration: desc,
                            interfaces: Vec::new(),
                            other_descriptors: Vec::new(),
                        });
                    }
                }
                0x04 => {
                    if let Some(iface) = current_iface.take() {
                        if let Some(tree) = config_tree.as_mut() {
                            tree.interfaces.push(iface);
                        }
                    }
                    if let Some(desc) = InterfaceDescriptor::parse(chunk) {
                        current_iface = Some(UsbInterfaceTree {
                            interface: desc,
                            endpoints: Vec::new(),
                            other_descriptors: Vec::new(),
                        });
                    }
                }
                0x05 => {
                    if let Some(iface) = current_iface.as_mut() {
                        if let Some(desc) = EndpointDescriptor::parse(chunk) {
                            iface.endpoints.push(desc);
                        }
                    }
                }
                _ => {
                    if let Some(iface) = current_iface.as_mut() {
                        iface.other_descriptors.push(chunk.to_vec());
                    } else if let Some(tree) = config_tree.as_mut() {
                        tree.other_descriptors.push(chunk.to_vec());
                    }
                }
            }

            offset += b_length;
        }

        if let Some(iface) = current_iface {
            if let Some(tree) = config_tree.as_mut() {
                tree.interfaces.push(iface);
            }
        }

        config_tree
    }
}

use core::fmt;

impl fmt::Display for UsbConfigurationTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Configuration Descriptor: Value = {}, Power = {}mA, Interfaces = {}",
            self.configuration.b_configuration_value,
            self.configuration.b_max_power as u16 * 2, //given in val * 2mA
            self.configuration.b_num_interfaces
        )?;

        for other in &self.other_descriptors {
            let desc_type = other.get(1).copied().unwrap_or(0);
            writeln!(f, "  |- [Other Descriptor] Type: 0x{:02X}, Length: {}", desc_type, other.len())?;
        }

        for iface_tree in &self.interfaces {
            let iface = &iface_tree.interface;
            writeln!(
                f,
                "  |- Interface {}: Alt = {}, Class = 0x{:02X}, SubClass = 0x{:02X}, Protocol = 0x{:02X}",
                iface.b_interface_number,
                iface.b_alternate_setting,
                iface.b_interface_class,
                iface.b_interface_sub_class,
                iface.b_interface_protocol
            )?;

            for other in &iface_tree.other_descriptors {
                let desc_type = other.get(1).copied().unwrap_or(0);
                writeln!(f, "  |    |- [Class Specific Descriptor] Type: 0x{:02X}, Length: {}", desc_type, other.len())?;
            }

            for ep in &iface_tree.endpoints {
                let dir = if (ep.b_endpoint_address & 0x80) != 0 { "IN " } else { "OUT" };
                let ep_num = ep.b_endpoint_address & 0x0F;

                let ep_type = match ep.bm_attributes & 0x03 {
                    0x00 => "Control    ",
                    0x01 => "Isochronous",
                    0x02 => "Bulk       ",
                    0x03 => "Interrupt  ",
                    _    => "Unknown    ",
                };

                writeln!(
                    f,
                    "  |    |- Endpoint {}: {} ({}), MaxPacket = {}, Interval = {}",
                    ep_num,
                    dir,
                    ep_type,
                    ep.w_max_packet_size,
                    ep.b_interval
                )?;
            }
        }

        Ok(())
    }
}
