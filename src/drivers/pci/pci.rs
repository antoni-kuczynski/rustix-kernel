/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */
use crate::drivers::pci::pci_device::PciDevice;
use crate::drivers::pci::pci_io::{pci_read8, pci_read16};
use crate::drivers::usb;
use crate::vgaprintln;

const CFG_HEADER_TYPE: u32 = 0x0E;
const CFG_VENDOR_ID: u32 = 0x00;
const CFG_DEVICE_ID: u32 = 0x02;
const CFG_CLASS_CODE: u32 = 0x0B;
const CFG_SUBCLASS: u32 = 0x0A;
const CFG_PROG_IF: u32 = 0x09;

const INVALID_VENDOR_ID: u16 = 0xFFFF;

//PCI CLASS VALUES
const CLASS_CODE_SERIAL_BUS_CONTROLLER: u8 = 0x0C;

const SUBCLASS_USB_CONTROLLER: u8 = 0x03;

fn init_device(device: &PciDevice) {
    //INITIALIZE DEVICES

    //USB CONTROLLERS
    if device.class_code() == CLASS_CODE_SERIAL_BUS_CONTROLLER
        && device.sub_class() == SUBCLASS_USB_CONTROLLER
    {
        usb::init_usb_controller(&device)
    }
}

pub fn for_each_pci_device(mut callback: impl FnMut(PciDevice)) {
    for bus in 0..256 {
        for device in 0..32 {
            let function0 = pci_check_device(bus, device, 0);

            let Some(function0_dev) = function0 else {
                continue;
            };

            let is_multifunction = (function0_dev.header_type() & 0x80) != 0;

            callback(function0_dev);

            if is_multifunction {
                for function in 1..8 {
                    if let Some(dev) = pci_check_device(bus, device, function) {
                        callback(dev);
                    }
                }
            }
        }
    }
}

pub fn pci_init() {
    vgaprintln!("Initializing PCI devices...");

    for_each_pci_device(|dev| {
        init_device(&dev);
    });
}

fn pci_check_device(bus: u32, device: u32, function: u32) -> Option<PciDevice> {
    let base_dev_id = PciDevice::get_pci_id(bus, device, function);

    let vendor_id: u16 = pci_read16(base_dev_id, CFG_VENDOR_ID);

    if vendor_id == INVALID_VENDOR_ID {
        return None;
    }

    let device_id = pci_read16(base_dev_id, CFG_DEVICE_ID);

    let class_code = pci_read8(base_dev_id, CFG_CLASS_CODE);
    let sub_class = pci_read8(base_dev_id, CFG_SUBCLASS);
    let prog_info_byte = pci_read8(base_dev_id, CFG_PROG_IF);
    let header_type = pci_read8(base_dev_id, CFG_HEADER_TYPE);

    let dev_info = PciDevice::new(
        vendor_id,
        device_id,
        class_code,
        sub_class,
        prog_info_byte,
        header_type,
        base_dev_id,
    );
    Some(dev_info)
}
