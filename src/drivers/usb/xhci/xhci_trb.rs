#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]
/*
 * Created by Antoni Kuczyński
 * 01/06/2026
 */
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

    pub const TRB_ENABLE_SLOT: u8 = 9;
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

pub trait EventTrb: TrbTrait {

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

    /// Creates a typed Port Status Change Event TRB without checking the TRB type.
    ///
    /// Use this only after checking `Trb::trb_type()` or when the caller already knows the event
    /// ring entry is type 34.
    pub const unsafe fn new_unchecked(raw: Trb) -> Self {
        Self { raw }
    }

    /// Returns the wrapped raw TRB by value.
    pub const fn into_raw(self) -> Trb {
        self.raw
    }

    /// Returns the Port ID that generated the status change event.
    ///
    /// xHCI port IDs are one-based and correspond to the operational port register index plus one.
    pub fn read_port_id(&self) -> u8 {
        ((self.raw.parameter() & Self::PORT_ID_MASK) >> Self::PORT_ID_SHIFT) as u8
    }

    /// Returns `true` when this event references the given one-based xHCI Port ID.
    pub fn is_for_port(&self, port_id: u8) -> bool {
        self.read_port_id() == port_id
    }

    /// Returns the raw parameter field.
    ///
    /// For Port Status Change Event TRBs, the Port ID is encoded in bits 31:24.
    pub fn read_parameter(&self) -> u64 {
        self.raw.parameter()
    }

    /// Returns the raw status field.
    ///
    /// The common event TRB Completion Code is available through `read_completion_code`.
    pub fn read_status(&self) -> u32 {
        self.raw.status()
    }

    /// Returns the raw control field.
    ///
    /// The common event TRB type and Cycle bit are available through the `EventTrb` helpers.
    pub fn read_control(&self) -> u32 {
        self.raw.control()
    }
}

impl TrbTrait for PortStatusChangeEventTrb {
    const TRB_TYPE: u8 = Trb::TRB_PORT_STATUS_CHANGE_EVENT;

    fn raw(&self) -> &Trb {
        &self.raw
    }
}

impl EventTrb for PortStatusChangeEventTrb {

}

