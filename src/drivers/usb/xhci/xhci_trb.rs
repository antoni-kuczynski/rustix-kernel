#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */

pub struct TrbCompletionCode;

impl TrbCompletionCode {
    pub const INVALID: u8 = 0;
    pub const SUCCESS: u8 = 1;
    pub const DATA_BUFFER_ERROR: u8 = 2;
    pub const BABBLE_DETECTED_ERROR: u8 = 3;
    pub const USB_TRANSACTION_ERROR: u8 = 4;
    pub const TRB_ERROR: u8 = 5;
    pub const STALL_ERROR: u8 = 6;
    pub const RESOURCE_ERROR: u8 = 7;
    pub const BANDWIDTH_ERROR: u8 = 8;
    pub const NO_SLOTS_AVAILABLE_ERROR: u8 = 9;
    pub const INVALID_STREAM_TYPE_ERROR: u8 = 10;
    pub const SLOT_NOT_ENABLED_ERROR: u8 = 11;
    pub const ENDPOINT_NOT_ENABLED_ERROR: u8 = 12;
    pub const SHORT_PACKET: u8 = 13;
    pub const RING_UNDERRUN: u8 = 14;
    pub const RING_OVERRUN: u8 = 15;
    pub const VF_EVENT_RING_FULL_ERROR: u8 = 16;
    pub const PARAMETER_ERROR: u8 = 17;
    pub const BANDWIDTH_OVERRUN_ERROR: u8 = 18;
    pub const CONTEXT_STATE_ERROR: u8 = 19;
    pub const NO_PING_RESPONSE_ERROR: u8 = 20;
    pub const EVENT_RING_FULL_ERROR: u8 = 21;
    pub const INCOMPATIBLE_DEVICE_ERROR: u8 = 22;
    pub const MISSED_SERVICE_ERROR: u8 = 23;
    pub const COMMAND_RING_STOPPED: u8 = 24;
    pub const COMMAND_ABORTED: u8 = 25;
    pub const STOPPED: u8 = 26;
    pub const STOPPED_LENGTH_INVALID: u8 = 27;
    pub const STOPPED_SHORT_PACKET: u8 = 28;
    pub const MAX_EXIT_LATENCY_TOO_LARGE_ERROR: u8 = 29;
    pub const ISOCH_BUFFER_OVERRUN: u8 = 31;
    pub const EVENT_LOST_ERROR: u8 = 32;
    pub const UNDEFINED_ERROR: u8 = 33;
    pub const INVALID_STREAM_ID_ERROR: u8 = 34;
    pub const SECONDARY_BANDWIDTH_ERROR: u8 = 35;
    pub const SPLIT_TRANSACTION_ERROR: u8 = 36;
}

//=========================================
//  TRB
//=========================================
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Trb {
    pub parameter: u64, //pointer / value
    pub status: u32,    //length, residual...
    pub control: u32,   //type, cycle, flags
}

impl Trb {
    // ====== CONSTANTS ======
    const CYCLE_BIT: u32 = 1 << 0;
    const TOGGLE_CYCLE_BIT: u32 = 1 << 1;
    const CHAIN_BIT: u32 = 1 << 4;

    const TRB_TYPE_SHIFT: u32 = 10;
    const TRB_TYPE_MASK: u32 = 0x3F << Self::TRB_TYPE_SHIFT;

    // Length is usually bits 0–16 of status
    const LENGTH_MASK: u32 = 0x1FFFF;

    // ===== TRB TYPE CONSTANTS (Table 6‑91) =====
    pub const TRB_RESERVED0: u8 = 0;
    pub const TRB_NORMAL: u8 = 1;
    pub const TRB_SETUP_STAGE: u8 = 2;
    pub const TRB_DATA_STAGE: u8 = 3;
    pub const TRB_STATUS_STAGE: u8 = 4;
    pub const TRB_ISOCH: u8 = 5;
    pub const TRB_LINK: u8 = 6;
    pub const TRB_EVENT_DATA: u8 = 7;
    pub const TRB_NO_OP: u8 = 8;

    pub const TRB_ENABLE_SLOT_COMMAND: u8 = 9;
    pub const TRB_DISABLE_SLOT_COMMAND: u8 = 10;
    pub const TRB_ADDRESS_DEVICE_COMMAND: u8 = 11;
    pub const TRB_CONFIGURE_ENDPOINT_COMMAND: u8 = 12;
    pub const TRB_EVALUATE_CONTEXT_COMMAND: u8 = 13;
    pub const TRB_RESET_ENDPOINT: u8 = 14;
    pub const TRB_STOP_ENDPOINT_COMMAND: u8 = 15;
    pub const TRB_SET_DEQUEUE_PTR: u8 = 16;
    pub const TRB_RESET_DEVICE: u8 = 17;
    pub const TRB_FORCE_EVENT: u8 = 18;

    pub const TRB_NEGOTIATE_BW: u8 = 19;
    pub const TRB_SET_LTV: u8 = 20;
    pub const TRB_GET_PORT_BW: u8 = 21;
    pub const TRB_FORCE_HEADER: u8 = 22;
    pub const TRB_NO_OP_CMD: u8 = 23;
    pub const TRB_GET_EXT_PROP: u8 = 24;
    pub const TRB_SET_EXT_PROP: u8 = 25;

    pub const TRB_TRANSFER_EVENT: u8 = 32;
    pub const TRB_COMMAND_COMPLETION_EVENT: u8 = 33;
    pub const TRB_PORT_STATUS_CHANGE_EVENT: u8 = 34;
    pub const TRB_BW_REQUEST_EVENT: u8 = 35;
    pub const TRB_DOORBELL_EVENT: u8 = 36;
    pub const TRB_HOST_CONTROLLER_EVENT: u8 = 37;
    pub const TRB_DEVICE_NOTIFICATION_EVENT: u8 = 38;
    pub const TRB_MFINDEX_WRAP_EVENT: u8 = 39;


    pub const fn new() -> Self {
        Self {
            parameter: 0,
            status: 0,
            control: 0,
        }
    }

    pub const fn new_raw(parameter: u64, status: u32, control: u32) -> Self {
        Self {
            parameter,
            status,
            control,
        }
    }

    // ====== PARAMETER ======
    pub fn parameter(&self) -> u64 {
        self.parameter
    }

    pub fn set_parameter(&mut self, value: u64) {
        self.parameter = value;
    }

    // ====== LENGTH (status low bits) ======
    pub fn length(&self) -> u32 {
        self.status & Self::LENGTH_MASK
    }

    pub fn set_length(&mut self, len: u32) {
        self.status = (self.status & !Self::LENGTH_MASK) | (len & Self::LENGTH_MASK);
    }

    // ====== CYCLE BIT ======
    pub fn cycle(&self) -> bool {
        (self.control & Self::CYCLE_BIT) != 0
    }

    pub fn cycle_val(&self) -> u32 {
        self.control & Self::CYCLE_BIT
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        if cycle {
            self.control |= Self::CYCLE_BIT;
        } else {
            self.control &= !Self::CYCLE_BIT;
        }
    }

    pub fn set_toggle_cycle(&mut self, toggle: bool) {
        if toggle {
            self.control |= Self::TOGGLE_CYCLE_BIT;
        } else {
            self.control &= !Self::TOGGLE_CYCLE_BIT;
        }
    }

    // ====== CHAIN BIT ======
    pub fn chain(&self) -> bool {
        (self.control & Self::CHAIN_BIT) != 0
    }

    pub fn set_chain(&mut self, chain: bool) {
        if chain {
            self.control |= Self::CHAIN_BIT;
        } else {
            self.control &= !Self::CHAIN_BIT;
        }
    }

    // ====== COMMON FLAGS & FIELDS ======
    pub fn ent(&self) -> bool {
        (self.control & (1 << 1)) != 0
    }

    pub fn set_ent(&mut self, ent: bool) {
        if ent {
            self.control |= 1 << 1;
        } else {
            self.control &= !(1 << 1);
        }
    }

    pub fn ioc(&self) -> bool {
        (self.control & (1 << 5)) != 0
    }

    pub fn set_ioc(&mut self, ioc: bool) {
        if ioc {
            self.control |= 1 << 5;
        } else {
            self.control &= !(1 << 5);
        }
    }

    pub fn direction(&self) -> u8 {
        ((self.control >> 16) & 0x1) as u8
    }

    pub fn set_direction(&mut self, direction: u8) {
        self.control = (self.control & !(1 << 16)) | (((direction as u32) & 0x1) << 16);
    }

    pub fn interrupter_target(&self) -> u16 {
        ((self.status >> 22) & 0x3FF) as u16
    }

    pub fn set_interrupter_target(&mut self, target: u16) {
        self.status = (self.status & !(0x3FF << 22)) | (((target as u32) & 0x3FF) << 22);
    }

    pub fn slot_id(&self) -> u8 {
        ((self.control >> 24) & 0xFF) as u8
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.control = (self.control & !(0xFF << 24)) | ((slot_id as u32) << 24);
    }

    // ====== TRB TYPE ======
    pub fn trb_type(&self) -> u8 {
        ((self.control & Self::TRB_TYPE_MASK) >> Self::TRB_TYPE_SHIFT) as u8
    }

    pub fn set_trb_type(&mut self, ty: u8) {
        let ctrl = self.control & !Self::TRB_TYPE_MASK;
        self.control = ctrl | ((ty as u32) << Self::TRB_TYPE_SHIFT);
    }

    #[inline]
    pub fn is_event_trb(&self) -> bool {
        let t = self.trb_type();
        t >= 32 && t <= 63
    }

    #[inline]
    pub fn is_command_trb(&self) -> bool {
        let t = self.trb_type();
        t >= 9 && t <= 31
    }

    #[inline]
    pub fn is_transfer_trb(&self) -> bool {
        let t = self.trb_type();
        t >= 1 && t <= 8
    }

    #[inline]
    pub fn is_control_transfer_trb(&self) -> bool {
        let t = self.trb_type();
        t == 2 || t == 3 || t == 4
    }

    #[inline]
    pub fn is_link_trb(&self) -> bool {
        self.trb_type() == 6
    }

    // ====== RAW CONTROL ======
    pub fn control(&self) -> u32 {
        self.control
    }

    pub fn set_control(&mut self, value: u32) {
        self.control = value;
    }

    // ====== RAW STATUS ======
    pub fn status(&self) -> u32 {
        self.status
    }

    pub fn set_status(&mut self, value: u32) {
        self.status = value;
    }

    pub fn try_as_port_status_change_event(
        &self,
    ) -> Result<PortStatusChangeEventTrb, TrbParseError> {
        PortStatusChangeEventTrb::new(*self)
    }

    pub fn try_as_command_completion_event(
        &self,
    ) -> Result<CommandCompletionEventTrb, TrbParseError> {
        CommandCompletionEventTrb::new(*self)
    }

    pub fn try_as_transfer_event(
        &self,
    ) -> Result<TransferEventTrb, TrbParseError> {
        TransferEventTrb::new(*self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrbParseError {
    UnexpectedType { expected: u8, actual: u8 },
}

pub trait TrbTrait {
    const TRB_TYPE: u8;

    fn raw(&self) -> &Trb;

    /// Returns the raw TRB type from the control field.
    fn read_trb_type(&self) -> u8 {
        self.raw().trb_type()
    }

    /// Returns the Cycle bit from the event TRB control field.
    fn read_cycle(&self) -> bool {
        self.raw().cycle()
    }

    /// Returns the Completion Code from status bits 31:24.
    fn read_completion_code(&self) -> u8 {
        ((self.raw().status() >> 24) & 0xFF) as u8
    }

    /// Returns `true` when the wrapped raw TRB has the type expected by this event wrapper.
    fn has_expected_type(&self) -> bool {
        self.read_trb_type() == Self::TRB_TYPE
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PortStatusChangeEventTrb {
    raw: Trb,
}

impl PortStatusChangeEventTrb {
    const PORT_ID_SHIFT: u64 = 24;
    const PORT_ID_MASK: u64 = 0xFF << Self::PORT_ID_SHIFT;

    /// Creates a typed Port Status Change Event TRB from a raw TRB.
    ///
    /// This accepts only TRB type 34. The raw TRB is copied, so the wrapper does not borrow the
    /// event ring memory.
    pub fn new(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_PORT_STATUS_CHANGE_EVENT {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_PORT_STATUS_CHANGE_EVENT,
                actual,
            });
        }

        Ok(Self { raw })
    }

    pub fn port_id(&self) -> u8 {
        ((self.raw.parameter() & Self::PORT_ID_MASK) >> Self::PORT_ID_SHIFT) as u8
    }
}

impl TrbTrait for PortStatusChangeEventTrb {
    const TRB_TYPE: u8 = Trb::TRB_PORT_STATUS_CHANGE_EVENT;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}



#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct EnableSlotCommandTrb {
    raw: Trb,
}

impl EnableSlotCommandTrb {
    const SLOT_TYPE_SHIFT: u32 = 16;
    const SLOT_TYPE_MASK: u32 = 0x1F << Self::SLOT_TYPE_SHIFT;

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_ENABLE_SLOT_COMMAND {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_ENABLE_SLOT_COMMAND,
                actual,
            });
        }

        Ok(Self { raw })
    }

    pub fn new_command(slot_type: u8) -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_ENABLE_SLOT_COMMAND);

        let mut command = Self { raw };
        command.set_slot_type(slot_type);

        command
    }

    pub fn slot_type(&self) -> u8 {
        ((self.raw.control() & Self::SLOT_TYPE_MASK) >> Self::SLOT_TYPE_SHIFT) as u8
    }

    pub fn set_slot_type(&mut self, slot_type: u8) {
        let mut ctrl = self.raw.control();
        ctrl &= !Self::SLOT_TYPE_MASK;
        ctrl |= ((slot_type as u32) << Self::SLOT_TYPE_SHIFT) & Self::SLOT_TYPE_MASK;

        self.raw.set_control(ctrl);
    }
}

impl TrbTrait for EnableSlotCommandTrb {
    const TRB_TYPE: u8 = Trb::TRB_ENABLE_SLOT_COMMAND;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}


#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct CommandCompletionEventTrb {
    raw: Trb,
}

impl CommandCompletionEventTrb {
    const COMP_PARAM_MASK: u32 = 0x00FF_FFFF;
    const COMP_CODE_SHIFT: u32 = 24;
    const COMP_CODE_MASK: u32 = 0xFF << Self::COMP_CODE_SHIFT;
    const VF_ID_SHIFT: u32 = 16;
    const VF_ID_MASK: u32 = 0xFF << Self::VF_ID_SHIFT;

    pub fn new(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_COMMAND_COMPLETION_EVENT {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_COMMAND_COMPLETION_EVENT,
                actual,
            });
        }

        Ok(Self { raw })
    }

    pub fn command_trb_pointer(&self) -> u64 {
        self.raw.parameter() & !0xF
    }

    pub fn command_completion_parameter(&self) -> u32 {
        self.raw.status() & Self::COMP_PARAM_MASK
    }

    pub fn completion_code(&self) -> u8 {
        ((self.raw.status() & Self::COMP_CODE_MASK) >> Self::COMP_CODE_SHIFT) as u8
    }

    pub fn vf_id(&self) -> u8 {
        ((self.raw.control() & Self::VF_ID_MASK) >> Self::VF_ID_SHIFT) as u8
    }

    pub fn slot_id(&self) -> u8 {
        self.raw.slot_id()
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }
}

impl TrbTrait for CommandCompletionEventTrb {
    const TRB_TYPE: u8 = Trb::TRB_COMMAND_COMPLETION_EVENT;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}


#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct AddressDeviceCommandTrb {
    raw: Trb,
}

impl AddressDeviceCommandTrb {
    const BSR_SHIFT: u32 = 9;
    const BSR_MASK: u32 = 1 << Self::BSR_SHIFT;

    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(11);
        Self { raw }
    }

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != 11 {
            return Err(TrbParseError::UnexpectedType {
                expected: 11,
                actual,
            });
        }

        Ok(Self { raw })
    }

    pub fn input_context_pointer(&self) -> u64 {
        self.raw.parameter() & !0xF
    }

    pub fn set_input_context_pointer(&mut self, ptr: u64) {
        self.raw.set_parameter(ptr & !0xF);
    }

    pub fn bsr(&self) -> bool {
        (self.raw.control() & Self::BSR_MASK) != 0
    }

    pub fn set_bsr(&mut self, bsr: bool) {
        let mut control = self.raw.control();
        if bsr {
            control |= Self::BSR_MASK;
        } else {
            control &= !Self::BSR_MASK;
        }
        self.raw.set_control(control);
    }

    pub fn slot_id(&self) -> u8 {
        self.raw.slot_id()
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.raw.set_slot_id(slot_id);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }
}

impl TrbTrait for AddressDeviceCommandTrb {
    const TRB_TYPE: u8 = 11;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}


pub struct TransferEventTrb {
    trb: Trb,
}

impl TransferEventTrb {
    pub fn new(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_TRANSFER_EVENT {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_TRANSFER_EVENT,
                actual,
            });
        }

        Ok(Self { trb: raw })
    }

    pub fn from_trb(trb: Trb) -> Self {
        Self { trb }
    }

    pub fn raw(&self) -> &Trb {
        &self.trb
    }

    pub fn set_trb_pointer(&mut self, pointer: u64) {
        self.trb.set_parameter(pointer);
    }

    pub fn trb_pointer(&self) -> u64 {
        self.trb.parameter()
    }

    pub fn set_transfer_length(&mut self, length: u32) {
        let mut status = self.trb.status();
        status &= !0xFFFFFF;
        status |= length & 0xFFFFFF;
        self.trb.set_status(status);
    }

    pub fn transfer_length(&self) -> u32 {
        self.trb.status() & 0xFFFFFF
    }

    pub fn set_completion_code(&mut self, code: u8) {
        let mut status = self.trb.status();
        status &= !(0xFF << 24);
        status |= (code as u32) << 24;
        self.trb.set_status(status);
    }

    pub fn completion_code(&self) -> u8 {
        ((self.trb.status() >> 24) & 0xFF) as u8
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.trb.set_cycle(cycle);
    }

    pub fn cycle(&self) -> bool {
        self.trb.cycle()
    }

    pub fn set_ed(&mut self, ed: bool) {
        let mut control = self.trb.control();
        if ed {
            control |= 1 << 2;
        } else {
            control &= !(1 << 2);
        }
        self.trb.set_control(control);
    }

    pub fn ed(&self) -> bool {
        (self.trb.control() & (1 << 2)) != 0
    }

    pub fn set_endpoint_id(&mut self, ep_id: u8) {
        let mut control = self.trb.control();
        control &= !(0x1F << 16);
        control |= ((ep_id as u32) & 0x1F) << 16;
        self.trb.set_control(control);
    }

    pub fn endpoint_id(&self) -> u8 {
        ((self.trb.control() >> 16) & 0x1F) as u8
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.trb.set_slot_id(slot_id);
    }

    pub fn slot_id(&self) -> u8 {
        self.trb.slot_id()
    }
}




pub struct StatusStageTrb {
    trb: Trb,
}

impl StatusStageTrb {
    pub fn new() -> Self {
        let mut trb = Trb::new();
        trb.set_trb_type(Trb::TRB_STATUS_STAGE);
        Self { trb }
    }

    pub fn raw(&self) -> &Trb {
        &self.trb
    }

    pub fn set_interrupter_target(&mut self, target: u16) {
        self.trb.set_interrupter_target(target);
    }

    pub fn interrupter_target(&self) -> u16 {
        self.trb.interrupter_target()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.trb.set_cycle(cycle);
    }

    pub fn cycle(&self) -> bool {
        self.trb.cycle()
    }

    pub fn set_ent(&mut self, ent: bool) {
        self.trb.set_ent(ent);
    }

    pub fn ent(&self) -> bool {
        self.trb.ent()
    }

    pub fn set_chain(&mut self, chain: bool) {
        self.trb.set_chain(chain);
    }

    pub fn chain(&self) -> bool {
        self.trb.chain()
    }

    pub fn set_ioc(&mut self, ioc: bool) {
        self.trb.set_ioc(ioc);
    }

    pub fn ioc(&self) -> bool {
        self.trb.ioc()
    }

    pub fn set_direction(&mut self, direction: u8) {
        self.trb.set_direction(direction);
    }

    pub fn direction(&self) -> u8 {
        self.trb.direction()
    }
}




pub struct DataStageTrb {
    trb: Trb,
}

impl DataStageTrb {
    pub fn new() -> Self {
        let mut trb = Trb::new();
        trb.set_trb_type(Trb::TRB_DATA_STAGE);
        Self { trb }
    }

    pub fn raw(&self) -> &Trb {
        &self.trb
    }

    pub fn set_data_buffer(&mut self, buffer_ptr: u64) {
        self.trb.set_parameter(buffer_ptr);
    }

    pub fn data_buffer(&self) -> u64 {
        self.trb.parameter()
    }

    pub fn set_transfer_length(&mut self, length: u32) {
        self.trb.set_length(length);
    }

    pub fn transfer_length(&self) -> u32 {
        self.trb.length()
    }

    pub fn set_td_size(&mut self, size: u8) {
        let mut status = self.trb.status();
        status &= !(0x1F << 17);
        status |= ((size as u32) & 0x1F) << 17;
        self.trb.set_status(status);
    }

    pub fn td_size(&self) -> u8 {
        ((self.trb.status() >> 17) & 0x1F) as u8
    }

    pub fn set_interrupter_target(&mut self, target: u16) {
        self.trb.set_interrupter_target(target);
    }

    pub fn interrupter_target(&self) -> u16 {
        self.trb.interrupter_target()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.trb.set_cycle(cycle);
    }

    pub fn cycle(&self) -> bool {
        self.trb.cycle()
    }

    pub fn set_ent(&mut self, ent: bool) {
        self.trb.set_ent(ent);
    }

    pub fn ent(&self) -> bool {
        self.trb.ent()
    }

    pub fn set_isp(&mut self, isp: bool) {
        let mut control = self.trb.control();
        if isp {
            control |= 1 << 2;
        } else {
            control &= !(1 << 2);
        }
        self.trb.set_control(control);
    }

    pub fn isp(&self) -> bool {
        (self.trb.control() & (1 << 2)) != 0
    }

    pub fn set_ns(&mut self, ns: bool) {
        let mut control = self.trb.control();
        if ns {
            control |= 1 << 3;
        } else {
            control &= !(1 << 3);
        }
        self.trb.set_control(control);
    }

    pub fn ns(&self) -> bool {
        (self.trb.control() & (1 << 3)) != 0
    }

    pub fn set_chain(&mut self, chain: bool) {
        self.trb.set_chain(chain);
    }

    pub fn chain(&self) -> bool {
        self.trb.chain()
    }

    pub fn set_ioc(&mut self, ioc: bool) {
        self.trb.set_ioc(ioc);
    }

    pub fn ioc(&self) -> bool {
        self.trb.ioc()
    }

    pub fn set_idt(&mut self, idt: bool) {
        let mut control = self.trb.control();
        if idt {
            control |= 1 << 6;
        } else {
            control &= !(1 << 6);
        }
        self.trb.set_control(control);
    }

    pub fn idt(&self) -> bool {
        (self.trb.control() & (1 << 6)) != 0
    }

    pub fn set_direction(&mut self, direction: u8) {
        self.trb.set_direction(direction);
    }

    pub fn direction(&self) -> u8 {
        self.trb.direction()
    }
}




#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct DisableSlotCommandTrb {
    raw: Trb,
}

impl DisableSlotCommandTrb {
    pub fn new(slot_id: u8) -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_DISABLE_SLOT_COMMAND);

        let mut command = Self { raw };
        command.set_slot_id(slot_id);

        command
    }

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_DISABLE_SLOT_COMMAND {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_DISABLE_SLOT_COMMAND,
                actual,
            });
        }

        Ok(Self { raw })
    }

    pub fn slot_id(&self) -> u8 {
        self.raw.slot_id()
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.raw.set_slot_id(slot_id);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }
}

impl TrbTrait for DisableSlotCommandTrb {
    const TRB_TYPE: u8 = Trb::TRB_DISABLE_SLOT_COMMAND;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}




#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SetupTransferType {
    NoDataStage = 0,
    OutDataStage = 2,
    InDataStage = 3,
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct SetupStageTrb {
    raw: Trb,
}

impl SetupStageTrb {
    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_SETUP_STAGE);

        let mut command = Self { raw };
        command.set_transfer_length(8);
        command.set_idt(true);

        command
    }

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_SETUP_STAGE {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_SETUP_STAGE,
                actual,
            });
        }
        Ok(Self { raw })
    }

    pub fn bm_request_type(&self) -> u8 {
        (self.raw.parameter() & 0xFF) as u8
    }

    pub fn set_bm_request_type(&mut self, val: u8) {
        let mut param = self.raw.parameter();
        param = (param & !0xFF_u64) | (val as u64);
        self.raw.set_parameter(param);
    }

    pub fn b_request(&self) -> u8 {
        ((self.raw.parameter() >> 8) & 0xFF) as u8
    }

    pub fn set_b_request(&mut self, val: u8) {
        let mut param = self.raw.parameter();
        param = (param & !(0xFF_u64 << 8)) | ((val as u64) << 8);
        self.raw.set_parameter(param);
    }

    pub fn w_value(&self) -> u16 {
        ((self.raw.parameter() >> 16) & 0xFFFF) as u16
    }

    pub fn set_w_value(&mut self, val: u16) {
        let mut param = self.raw.parameter();
        param = (param & !(0xFFFF_u64 << 16)) | ((val as u64) << 16);
        self.raw.set_parameter(param);
    }

    pub fn w_index(&self) -> u16 {
        ((self.raw.parameter() >> 32) & 0xFFFF) as u16
    }

    pub fn set_w_index(&mut self, val: u16) {
        let mut param = self.raw.parameter();
        param = (param & !(0xFFFF_u64 << 32)) | ((val as u64) << 32);
        self.raw.set_parameter(param);
    }

    pub fn w_length(&self) -> u16 {
        ((self.raw.parameter() >> 48) & 0xFFFF) as u16
    }

    pub fn set_w_length(&mut self, val: u16) {
        let mut param = self.raw.parameter();
        param = (param & !(0xFFFF_u64 << 48)) | ((val as u64) << 48);
        self.raw.set_parameter(param);
    }

    pub fn transfer_length(&self) -> u32 {
        self.raw.length()
    }

    pub fn set_transfer_length(&mut self, val: u32) {
        self.raw.set_length(val);
    }

    pub fn interrupter_target(&self) -> u16 {
        self.raw.interrupter_target()
    }

    pub fn set_interrupter_target(&mut self, val: u16) {
        self.raw.set_interrupter_target(val);
    }

    pub fn idt(&self) -> bool {
        (self.raw.control() & (1 << 6)) != 0
    }

    pub fn set_idt(&mut self, idt: bool) {
        let mut control = self.raw.control();
        if idt {
            control |= 1 << 6;
        } else {
            control &= !(1 << 6);
        }
        self.raw.set_control(control);
    }

    pub fn trt(&self) -> SetupTransferType {
        let val = (self.raw.control() >> 16) & 0x3;
        match val {
            0 => SetupTransferType::NoDataStage,
            2 => SetupTransferType::OutDataStage,
            3 => SetupTransferType::InDataStage,
            _ => SetupTransferType::NoDataStage,
        }
    }

    pub fn set_trt(&mut self, trt: SetupTransferType) {
        let mut control = self.raw.control();
        control = (control & !(0x3 << 16)) | ((trt as u32) << 16);
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }

    pub fn ioc(&self) -> bool {
        self.raw.ioc()
    }

    pub fn set_ioc(&mut self, ioc: bool) {
        self.raw.set_ioc(ioc);
    }
}

impl TrbTrait for SetupStageTrb {
    const TRB_TYPE: u8 = Trb::TRB_SETUP_STAGE;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}




#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct EvaluateContextCmdTrb {
    raw: Trb,
}

impl EvaluateContextCmdTrb {
    pub const TRB_TYPE: u8 = 13;

    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Self::TRB_TYPE);

        Self { raw }
    }

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Self::TRB_TYPE {
            return Err(TrbParseError::UnexpectedType {
                expected: Self::TRB_TYPE,
                actual,
            });
        }
        Ok(Self { raw })
    }

    pub fn input_context_pointer(&self) -> u64 {
        self.raw.parameter() & !0xF_u64
    }

    pub fn set_input_context_pointer(&mut self, ptr: u64) {
        self.raw.set_parameter(ptr & !0xF_u64);
    }

    pub fn slot_id(&self) -> u8 {
        ((self.raw.control() >> 24) & 0xFF) as u8
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        let mut control = self.raw.control();
        control = (control & !(0xFF << 24)) | ((slot_id as u32) << 24);
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }
}

impl TrbTrait for EvaluateContextCmdTrb {
    const TRB_TYPE: u8 = Self::TRB_TYPE;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}



#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct NormalTrb {
    raw: Trb,
}

impl NormalTrb {
    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_NORMAL);

        Self { raw }
    }

    pub fn from_raw(raw: Trb) -> Result<Self, TrbParseError> {
        let actual = raw.trb_type();
        if actual != Trb::TRB_NORMAL {
            return Err(TrbParseError::UnexpectedType {
                expected: Trb::TRB_NORMAL,
                actual,
            });
        }
        Ok(Self { raw })
    }

    pub fn data_buffer(&self) -> u64 {
        self.raw.parameter()
    }

    pub fn set_data_buffer(&mut self, val: u64) {
        self.raw.set_parameter(val);
    }

    pub fn transfer_length(&self) -> u32 {
        self.raw.length()
    }

    pub fn set_transfer_length(&mut self, val: u32) {
        self.raw.set_length(val);
    }

    pub fn interrupter_target(&self) -> u16 {
        self.raw.interrupter_target()
    }

    pub fn set_interrupter_target(&mut self, val: u16) {
        self.raw.set_interrupter_target(val);
    }

    pub fn ent(&self) -> bool {
        (self.raw.control() & (1 << 1)) != 0
    }

    pub fn set_ent(&mut self, ent: bool) {
        let mut control = self.raw.control();
        if ent {
            control |= 1 << 1;
        } else {
            control &= !(1 << 1);
        }
        self.raw.set_control(control);
    }

    pub fn isp(&self) -> bool {
        (self.raw.control() & (1 << 2)) != 0
    }

    pub fn set_interrupt_on_short_packet(&mut self, isp: bool) {
        let mut control = self.raw.control();
        if isp {
            control |= 1 << 2;
        } else {
            control &= !(1 << 2);
        }
        self.raw.set_control(control);
    }

    pub fn ns(&self) -> bool {
        (self.raw.control() & (1 << 3)) != 0
    }

    pub fn set_ns(&mut self, ns: bool) {
        let mut control = self.raw.control();
        if ns {
            control |= 1 << 3;
        } else {
            control &= !(1 << 3);
        }
        self.raw.set_control(control);
    }

    pub fn chain(&self) -> bool {
        (self.raw.control() & (1 << 4)) != 0
    }

    pub fn set_chain(&mut self, chain: bool) {
        let mut control = self.raw.control();
        if chain {
            control |= 1 << 4;
        } else {
            control &= !(1 << 4);
        }
        self.raw.set_control(control);
    }

    pub fn bei(&self) -> bool {
        (self.raw.control() & (1 << 9)) != 0
    }

    pub fn set_bei(&mut self, bei: bool) {
        let mut control = self.raw.control();
        if bei {
            control |= 1 << 9;
        } else {
            control &= !(1 << 9);
        }
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }

    pub fn ioc(&self) -> bool {
        self.raw.ioc()
    }

    pub fn set_interrupt_on_completion(&mut self, ioc: bool) {
        self.raw.set_ioc(ioc);
    }
}

impl TrbTrait for NormalTrb {
    const TRB_TYPE: u8 = Trb::TRB_NORMAL;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}


#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct StopEndpointCommandTrb {
    raw: Trb,
}

impl StopEndpointCommandTrb {
    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_STOP_ENDPOINT_COMMAND);

        Self { raw }
    }

    pub fn endpoint_id(&self) -> u8 {
        ((self.raw.control() >> 16) & 0x1F) as u8
    }

    pub fn set_endpoint_id(&mut self, id: u8) {
        let mut control = self.raw.control();
        control = (control & !(0x1F << 16)) | ((id as u32) << 16);
        self.raw.set_control(control);
    }

    pub fn suspend(&self) -> bool {
        (self.raw.control() & (1 << 23)) != 0
    }

    pub fn set_suspend(&mut self, suspend: bool) {
        let mut control = self.raw.control();
        if suspend {
            control |= 1 << 23;
        } else {
            control &= !(1 << 23);
        }
        self.raw.set_control(control);
    }

    pub fn slot_id(&self) -> u8 {
        ((self.raw.control() >> 24) & 0xFF) as u8
    }

    pub fn set_slot_id(&mut self, id: u8) {
        let mut control = self.raw.control();
        control = (control & !(0xFF << 24)) | ((id as u32) << 24);
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }
}

impl TrbTrait for StopEndpointCommandTrb {
    const TRB_TYPE: u8 = Trb::TRB_STOP_ENDPOINT_COMMAND;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}



#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ConfigureEndpointCommandTrb {
    raw: Trb,
}

impl ConfigureEndpointCommandTrb {
    pub fn new() -> Self {
        let mut raw = Trb::new();
        raw.set_trb_type(Trb::TRB_CONFIGURE_ENDPOINT_COMMAND);

        Self { raw }
    }

    pub fn input_context_pointer(&self) -> u64 {
        self.raw.parameter() & !0xF
    }

    pub fn set_input_context_pointer(&mut self, ptr: u64) {
        self.raw.set_parameter(ptr & !0xF);
    }

    pub fn deconfigure(&self) -> bool {
        (self.raw.control() & (1 << 9)) != 0
    }

    pub fn set_deconfigure(&mut self, dc: bool) {
        let mut control = self.raw.control();
        if dc {
            control |= 1 << 9;
        } else {
            control &= !(1 << 9);
        }
        self.raw.set_control(control);
    }

    pub fn slot_id(&self) -> u8 {
        ((self.raw.control() >> 24) & 0xFF) as u8
    }

    pub fn set_slot_id(&mut self, id: u8) {
        let mut control = self.raw.control();
        control = (control & !(0xFF << 24)) | ((id as u32) << 24);
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        self.raw.cycle()
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        self.raw.set_cycle(cycle);
    }
}

impl TrbTrait for ConfigureEndpointCommandTrb {
    const TRB_TYPE: u8 = Trb::TRB_CONFIGURE_ENDPOINT_COMMAND;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}