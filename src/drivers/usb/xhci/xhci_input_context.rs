/*
 * Created by Antoni Kuczyński
 * 08/07/2026
 */
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct InputControlContext {
    drop_flags: u32,
    add_flags: u32,
    reserved: [u32; 5],
    config_info: u32,
}

impl InputControlContext {
    const CONFIG_VAL_MASK: u32 = 0x0000_00FF;
    const INTERFACE_NUM_MASK: u32 = 0x0000_FF00;
    const ALT_SETTING_MASK: u32 = 0x00FF_0000;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_context(&mut self, index: u8) {
        assert!(index <= 31, "Add Context index must be <= 31");
        self.add_flags |= 1 << index;
    }

    pub fn drop_context(&mut self, index: u8) {
        assert!(index >= 2 && index <= 31, "Drop Context index must be between 2 and 31");
        self.drop_flags |= 1 << index;
    }

    pub fn add_slot_context(&mut self) {
        self.add_context(1);
    }

    pub fn add_ep0_context(&mut self) {
        self.add_context(2);
    }

    pub fn set_configuration_value(&mut self, val: u8) {
        self.config_info &= !Self::CONFIG_VAL_MASK;
        self.config_info |= val as u32;
    }

    pub fn set_interface_number(&mut self, val: u8) {
        self.config_info &= !Self::INTERFACE_NUM_MASK;
        self.config_info |= (val as u32) << 8;
    }

    pub fn set_alternate_setting(&mut self, val: u8) {
        self.config_info &= !Self::ALT_SETTING_MASK;
        self.config_info |= (val as u32) << 16;
    }
}


#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct InputContext {
    data: [u8; 2112],
}

impl InputContext {
    pub fn new() -> Self {
        Self { data: [0; 2112] }
    }

    pub fn control(&mut self) -> &mut InputControlContext {
        unsafe { &mut *(self.data.as_mut_ptr() as *mut InputControlContext) }
    }

    pub fn slot<T>(&mut self, context_size: u32) -> &mut T {
        assert!(context_size == 32 || context_size == 64);
        unsafe {
            let ptr = (self.data.as_mut_ptr() as *mut u8).add(context_size as usize);
            &mut *(ptr as *mut T)
        }
    }

    pub fn slot_ref<T>(&self, context_size: u32) -> &T {
        assert!(context_size == 32 || context_size == 64);
        unsafe {
            let ptr = (self.data.as_ptr() as *const u8).add(context_size as usize);
            &*(ptr as *const T)
        }
    }

    pub fn endpoint<T>(&mut self, ici: usize, context_size: u32) -> &mut T {
        unsafe { &mut *(self.data.as_mut_ptr().add(ici * context_size as usize) as *mut T) }
    }

    pub fn endpoint_ref<T>(&self, ici: usize, context_size: u32) -> &T {
        unsafe { &*(self.data.as_ptr().add(ici * context_size as usize) as *const T) }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }
}