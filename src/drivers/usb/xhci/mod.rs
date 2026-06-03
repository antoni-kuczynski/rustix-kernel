pub mod xhci;
mod xhci_endpoint_context;
mod xhci_ext_cap;
mod xhci_msix;
mod xhci_portsc;
mod xhci_slot_context;
mod xhci_trb;
/*
Base
│
├─ Capability Registers        (offset 0x00)
│   └─ CAPLENGTH (offset 0x00)
│
├─ Operational Registers       (offset = CAPLENGTH)
│
├─ Runtime Registers           (offset = RTSOFF)
│
└─ Doorbell Registers          (offset = DBOFF)

op_base = base + caplength


The Runtime Base shall be 32-
byte aligned and is calculated by adding the value Runtime Register Space
Offset register (refer to Section 5.3.8) to the Capability Base address. All
Runtime registers are multiples of 32 bits in length.

runtime_base = RTSOFF +

 */

//CAPABILITY REGS
const CAP_REG_CAPLENGTH: u8 = 0x00;
const CAP_REG_HCSPARAMS1: u8 = 0x04;
const CAP_REG_HCCPARAMS1: u8 = 0x10;
const CAP_REG_RTSOFF: u8 = 0x18;

//OPERATIONAL REGS
const OP_REG_USBCMD: u8 = 0x00;
const OP_REG_USBSTS: u8 = 0x04;
const OP_REG_CONFIG: u8 = 0x38;
const OP_REG_DCBAAP: u8 = 0x30;
const OP_REG_CRCR: u8 = 0x18;

//RUNTIME REGISTERS
const RT_IMAN: u8 = 0x20;
const RT_ERSTSZ: u8 = 0x28;
const RT_ERSTBA: u8 = 0x30;
const RT_ERDP: u8 = 0x38;

const XHCI_CONTROLLER_NOT_READY: u32 = 1 << 11;
const XHCI_STATUS_HALTED: u32 = 1 << 0;
const COMMAND_RING_TRBS: usize = 256;
const EVENT_RING_TRBS: usize = 256;

//==================================================================================================
const USB_CMD_RUN_STOP: u32 = 1 << 0;
const USB_CMD_HOST_CONTROLLER_RESET: u32 = 1 << 1;
const USB_CMD_INTERRUPTER_ENABLE: u32 = 1 << 2;
const INTERRUPTER_REGISTER_STRIDE: u64 = 0x20;
const INTERRUPTER_MANAGEMENT_ENABLE: u32 = 1 << 1;
//==================================================================================================
pub const HCCPARAMS1_XECP_MASK: u32 = 0xFFFF_0000;
pub const HCCPARAMS1_XECP_SHIFT: u32 = 16;
