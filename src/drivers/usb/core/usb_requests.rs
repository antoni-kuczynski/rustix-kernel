/*
 * Created by Antoni Kuczyński
 * 14/11/2026
 */
use alloc::sync::Arc;
use crate::drivers::usb::core::irq_mutex::IrqMutex;
use crate::drivers::usb::core::usb_transfers::{
    UsbDevice, UsbSetupPacket, UsbTransferCallback, UsbTransferDirection, UsbTransferRequest,
    UsbTransferStatus, UsbTransferType,
};
use crate::drivers::usb::{UsbBRequest, UsbBmRequestType, WValue};
use crate::memory::dma::dma_alloc_zeroed;

const DMA_BUFFER_ALIGNMENT: usize = 64;
const DEFAULT_CONTROL_ENDPOINT_ADDRESS: u8 = 0;
const FIRST_DESCRIPTOR_INDEX: u8 = 0;
const DEVICE_DESCRIPTOR_LENGTH: u16 = 18;
const HUB_DESCRIPTOR_MAX_LENGTH: u16 = 128;
const HUB_PORT_STATUS_LENGTH: u16 = 4;
const HID_BOOT_PROTOCOL: u16 = 0;
const HID_IDLE_INDEFINITELY_ALL_REPORTS: u16 = 0;
const HUB_CLEAR_TT_BUFFER: u8 = 8;
const ENDPOINT_NUMBER_MASK: u8 = 0x0F;
const ENDPOINT_DIRECTION_IN: u8 = 0x80;

pub type UsbRequestHandle = Arc<IrqMutex<UsbTransferRequest>>;

fn descriptor_w_value(descriptor_type: u8, descriptor_index: u8) -> u16 {
    ((descriptor_type as u16) << 8) | descriptor_index as u16
}

fn control_request(
    device: Arc<UsbDevice>,
    setup_packet: UsbSetupPacket,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let data_length = setup_packet.w_length as usize;
    let is_device_to_host = setup_packet.bm_request_type & UsbBmRequestType::DIR_DEVICE_TO_HOST != 0;

    let dma_buffer = if data_length > 0 {
        Some(dma_alloc_zeroed(data_length, DMA_BUFFER_ALIGNMENT)?)
    } else {
        None
    };

    Some(Arc::new(IrqMutex::new(UsbTransferRequest {
        target_device: device,
        endpoint_address: DEFAULT_CONTROL_ENDPOINT_ADDRESS,
        transfer_direction: if is_device_to_host {
            UsbTransferDirection::DeviceToHost
        } else {
            UsbTransferDirection::HostToDevice
        },
        transfer_type: UsbTransferType::Control,
        setup_packet: Some(setup_packet),
        dma_buffer,
        data_buffer_length: data_length,
        status: UsbTransferStatus::Pending,
        bytes_transferred: 0,
        completion_callback: Some(callback),
    })))
}

pub fn usb_request_get_device_descriptor(
    device: Arc<UsbDevice>,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_DEVICE_TO_HOST,
            UsbBmRequestType::TYPE_STANDARD,
            UsbBmRequestType::REC_DEVICE,
        ),
        UsbBRequest::GET_DESCRIPTOR,
        descriptor_w_value(WValue::DESC_DEVICE, FIRST_DESCRIPTOR_INDEX),
        0,
        DEVICE_DESCRIPTOR_LENGTH,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_get_configuration_descriptor(
    device: Arc<UsbDevice>,
    length: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_DEVICE_TO_HOST,
            UsbBmRequestType::TYPE_STANDARD,
            UsbBmRequestType::REC_DEVICE,
        ),
        UsbBRequest::GET_DESCRIPTOR,
        descriptor_w_value(WValue::DESC_CONFIGURATION, FIRST_DESCRIPTOR_INDEX),
        0,
        length,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_set_configuration(
    device: Arc<UsbDevice>,
    configuration_value: u8,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_STANDARD,
            UsbBmRequestType::REC_DEVICE,
        ),
        UsbBRequest::SET_CONFIGURATION,
        configuration_value as u16,
        0,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_get_hub_descriptor(
    device: Arc<UsbDevice>,
    is_super_speed_hub: bool,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let descriptor_type = if is_super_speed_hub {
        WValue::DESC_SUPERSPEED_HUB
    } else {
        WValue::DESC_HUB
    };

    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_DEVICE_TO_HOST,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_DEVICE,
        ),
        UsbBRequest::GET_DESCRIPTOR,
        descriptor_w_value(descriptor_type, FIRST_DESCRIPTOR_INDEX),
        0,
        HUB_DESCRIPTOR_MAX_LENGTH,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_set_hub_port_feature(
    device: Arc<UsbDevice>,
    port_number: u16,
    feature_selector: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_OTHER,
        ),
        UsbBRequest::SET_FEATURE,
        feature_selector,
        port_number,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_clear_hub_port_feature(
    device: Arc<UsbDevice>,
    port_number: u16,
    feature_selector: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_OTHER,
        ),
        UsbBRequest::CLEAR_FEATURE,
        feature_selector,
        port_number,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_clear_hub_port_connection_change(
    device: Arc<UsbDevice>,
    port_number: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    usb_request_clear_hub_port_feature(device, port_number, WValue::C_PORT_CONNECTION, callback)
}

pub fn usb_request_get_hub_port_status(
    device: Arc<UsbDevice>,
    port_number: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_DEVICE_TO_HOST,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_OTHER,
        ),
        UsbBRequest::GET_STATUS,
        0,
        port_number,
        HUB_PORT_STATUS_LENGTH,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_clear_endpoint_halt(
    device: Arc<UsbDevice>,
    endpoint_address: u8,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_STANDARD,
            UsbBmRequestType::REC_ENDPOINT,
        ),
        UsbBRequest::CLEAR_FEATURE,
        WValue::FEATURE_ENDPOINT_HALT,
        endpoint_address as u16,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_clear_tt_buffer(
    hub: Arc<UsbDevice>,
    tt_port_number: u16,
    device_address: u8,
    endpoint_address: u8,
    endpoint_transfer_type: u8,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let endpoint_number = (endpoint_address & ENDPOINT_NUMBER_MASK) as u16;
    let is_direction_in = endpoint_address & ENDPOINT_DIRECTION_IN != 0;

    let w_value = endpoint_number
        | ((device_address as u16 & 0x7F) << 4)
        | ((endpoint_transfer_type as u16 & 0x3) << 11)
        | ((is_direction_in as u16) << 15);

    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_OTHER,
        ),
        HUB_CLEAR_TT_BUFFER,
        w_value,
        tt_port_number,
        0,
    );

    control_request(hub, setup_packet, callback)
}

pub fn hid_request_set_boot_protocol(
    device: Arc<UsbDevice>,
    interface_number: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_INTERFACE,
        ),
        UsbBRequest::HID_SET_PROTOCOL,
        HID_BOOT_PROTOCOL,
        interface_number,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn hid_request_set_idle_indefinitely(
    device: Arc<UsbDevice>,
    interface_number: u16,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let setup_packet = UsbSetupPacket::new(
        UsbBmRequestType::new(
            UsbBmRequestType::DIR_HOST_TO_DEVICE,
            UsbBmRequestType::TYPE_CLASS,
            UsbBmRequestType::REC_INTERFACE,
        ),
        UsbBRequest::HID_SET_IDLE,
        HID_IDLE_INDEFINITELY_ALL_REPORTS,
        interface_number,
        0,
    );

    control_request(device, setup_packet, callback)
}

pub fn usb_request_interrupt_in(
    device: Arc<UsbDevice>,
    endpoint_address: u8,
    buffer_length: usize,
    callback: UsbTransferCallback,
) -> Option<UsbRequestHandle> {
    let dma_buffer = dma_alloc_zeroed(buffer_length, DMA_BUFFER_ALIGNMENT)?;

    Some(Arc::new(IrqMutex::new(UsbTransferRequest {
        target_device: device,
        endpoint_address,
        transfer_direction: UsbTransferDirection::DeviceToHost,
        transfer_type: UsbTransferType::Interrupt,
        setup_packet: None,
        dma_buffer: Some(dma_buffer),
        data_buffer_length: buffer_length,
        status: UsbTransferStatus::Pending,
        bytes_transferred: 0,
        completion_callback: Some(callback),
    })))
}
