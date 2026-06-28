/*
 * Created by Antoni Kuczyński
 * 24/12/2025
 */
use alloc::vec;
use alloc::vec::Vec;
use spin::Once;
use x86_64::PhysAddr;
use crate::drivers::pci::pci_device::{PciDeviceHeader};
use crate::drivers::pci::pci_io::{pci_read16, pci_read8};
use crate::{kprintln, kprintln_ok};
use crate::drivers::acpi::acpi_tables::{acpi_get_sdt_table, ACPISignature};
use crate::drivers::acpi::tables::mcfg::{McfgAllocation, MCFG};
use crate::drivers::usb;
use crate::memory::ioremap::{ioremap_ext_permanent, IoAlloc};
use crate::memory::page_tables::PageSize;
use crate::memory::SizeUnit;

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


pub struct PciMmioInfo {
    pub mcfg_alloc: &'static McfgAllocation,
    pub io_alloc: IoAlloc
}


fn init_device(device: &PciDeviceHeader) {
    //INITIALIZE DEVICES

    //USB CONTROLLERS
    if device.class_code() == CLASS_CODE_SERIAL_BUS_CONTROLLER && device.sub_class() == SUBCLASS_USB_CONTROLLER {
        usb::init_usb_controller(&device)
    }
}

fn init_mmio(mcfg: &MCFG) {
    kprintln!(Info, "Using PCI enchanced configuration mechanism.");
    let allocations = mcfg.allocations();
    kprintln!(Debug, "MCFG allocations found on system: {}.", allocations.len());

    let mut mmio_maps: Vec<PciMmioInfo> = vec![];
    for alloc in allocations {
        let amount_of_buses = (alloc.end_bus_number - alloc.start_bus_number) as u64 + 1;
        let size = amount_of_buses * SizeUnit::Megabyte.as_u64(); //1mb per bus
        let phys_addr = PhysAddr::new(alloc.base_address);

        if phys_addr.as_u64() & (PageSize::Size4Kb.as_u64() - 1) != 0 {
            panic!("PCI enchanced configuration mechanism address is not aligned to 4096.");
        }

        let io_alloc = ioremap_ext_permanent(phys_addr, size, PageSize::Size4Kb.as_usize(), 0, PageSize::Size2Mb);
        let mmio_info = PciMmioInfo {
            mcfg_alloc: &allocations[0],
            io_alloc
        };
        mmio_maps.push(mmio_info);
    }

    mmio_maps.sort_by(|a,b| a.mcfg_alloc.start_bus_number.cmp(&b.mcfg_alloc.start_bus_number));
    PCI_MMIO_ALLOCS.call_once(|| mmio_maps);
}

pub fn pci_init() {
    let mcfg = if let Some(mcfg_addr) = acpi_get_sdt_table(ACPISignature::MCFG) {
        init_mmio(MCFG::new_from_ptr(mcfg_addr));
        Some(MCFG::new_from_ptr(mcfg_addr))
    } else {
        None
    };

    for bus in 0..256 {
        for device in 0..32 {
            let pci_id = PciDeviceHeader::get_pci_id(bus, device, 0);
            let header_type = pci_read8(pci_id, CFG_HEADER_TYPE);
            let function_count = if (header_type & 0x80) != 0 {
                8
            } else {
                1
            };

            for function in 0..function_count {
                let device = pci_check_device(bus, device, function);

                match device {
                    None => {},
                    Some(dev) => {
                        init_device(&dev)
                    }
                }

            }

        }
    }
    kprintln_ok!("Finished initializing PCI devices.");
}

fn pci_check_device(bus: u32, device: u32, function: u32) -> Option<PciDeviceHeader> {
    let base_dev_id = PciDeviceHeader::get_pci_id(bus, device, function);

    let vendor_id: u16 = pci_read16(base_dev_id, CFG_VENDOR_ID);

    if vendor_id == INVALID_VENDOR_ID {
        return None;
    }

    let device_id = pci_read16(base_dev_id, CFG_DEVICE_ID);

    let class_code = pci_read8(base_dev_id, CFG_CLASS_CODE);
    let sub_class = pci_read8(base_dev_id, CFG_SUBCLASS);
    let prog_info_byte = pci_read8(base_dev_id, CFG_PROG_IF);
    let header_type = pci_read8(base_dev_id, CFG_HEADER_TYPE);

    let dev_info = PciDeviceHeader::new(
        vendor_id, device_id, class_code, sub_class, prog_info_byte, header_type, base_dev_id
    );
    Some(dev_info)
}

pub static PCI_MMIO_ALLOCS: Once<Vec<PciMmioInfo>> = Once::new();