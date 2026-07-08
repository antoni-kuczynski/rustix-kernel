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
    pub const TRB_DISABLE_SLOT: u8 = 10;
    pub const TRB_ADDRESS_DEVICE: u8 = 11;
    pub const TRB_CONFIGURE_ENDPOINT: u8 = 12;
    pub const TRB_EVALUATE_CONTEXT: u8 = 13;
    pub const TRB_RESET_ENDPOINT: u8 = 14;
    pub const TRB_STOP_ENDPOINT: u8 = 15;
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
    const SLOT_ID_SHIFT: u32 = 24;
    const SLOT_ID_MASK: u32 = 0xFF << Self::SLOT_ID_SHIFT;

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
        ((self.raw.control() & Self::SLOT_ID_MASK) >> Self::SLOT_ID_SHIFT) as u8
    }

    pub fn cycle(&self) -> bool {
        (self.raw.control() & 1) != 0
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
    const TRB_TYPE_SHIFT: u32 = 10;
    const TRB_TYPE_MASK: u32 = 0x3F << Self::TRB_TYPE_SHIFT;
    const SLOT_ID_SHIFT: u32 = 24;
    const SLOT_ID_MASK: u32 = 0xFF << Self::SLOT_ID_SHIFT;

    pub fn new() -> Self {
        let mut trb = Self { raw: Trb::new() };
        trb.set_trb_type(11);
        trb
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
        ((self.raw.control() & Self::SLOT_ID_MASK) >> Self::SLOT_ID_SHIFT) as u8
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        let mut control = self.raw.control();
        control &= !Self::SLOT_ID_MASK;
        control |= (slot_id as u32) << Self::SLOT_ID_SHIFT;
        self.raw.set_control(control);
    }

    pub fn cycle(&self) -> bool {
        (self.raw.control() & 1) != 0
    }

    pub fn set_cycle(&mut self, cycle: bool) {
        let mut control = self.raw.control();
        if cycle {
            control |= 1;
        } else {
            control &= !1;
        }
        self.raw.set_control(control);
    }

    fn set_trb_type(&mut self, trb_type: u8) {
        let mut control = self.raw.control();
        control &= !Self::TRB_TYPE_MASK;
        control |= (trb_type as u32) << Self::TRB_TYPE_SHIFT;
        self.raw.set_control(control);
    }
}

impl TrbTrait for AddressDeviceCommandTrb {
    const TRB_TYPE: u8 = 11;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}
