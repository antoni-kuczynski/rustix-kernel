//  **VGA REGISTER VALUES**
//  *MODE 0x13 320x200px 256 colors (8bit)*
pub const VGA_13H_MISC_OUTPUT_REG: u8 = 0x63;
pub const VGA_13H_CRT_CONTROL_REGS: [u16; 18] = [
    0x5F00, //Horizontal total register (index 0x00)
    0x4F01, //Horizontal Display-Enable End Register (index 0x01)
    0x5002, //Start Horizontal Blanking Register (index 0x02)
    0x8203, //End Horizontal Blanking Register (index 0x03)
    0x5404, //Start Horizontal Retrace Pulse Register (index 0x04)
    0x8005, //End Horizontal Retrace Register (index 0x05)
    0xBF06, //Vertical Total Register (index 0x06)
    0x1F07, //Overflow Register (0x07)
    0x0008, //Preset row scan register (index 0x08)
    0x4109, //Maximum scan line regisyer (index 0x09)
    0x9C10, //Vertical Retrace Start Register (index 0x10)
    0x0E11, //Vertical Retrace End Register (index 0x11, is set first to unlock registers 0x00 to 0x07
    0x8F12, //Vertical Display-Enable End Register (0x12)
    0x2813, //Offset Register (0x13)
    0x4014, //Underline Location Register (index 0x14)
    0x9615, //Start Vertical Blanking Register (0x15)
    0xB916, //End Vertical Blanking Register (0x16)
    0xA317  //CRT Mode Control Register (0x17)
];

pub const VGA_13H_SEQUENCER_REGS: [u16; 5] = [
    0x0100, //Sequencer Address Register
    0x0101, //Clocking mode register (index 0x01)
    0x0F02, //Map mask register (index 0x02)
    0x0003, //Character map select register (index 0x03)
    0x0E04  //Memory mode register (index 0x04)
];

pub const VGA_13H_GRAPHICS_CONTROLLER_REGS: [u16; 9] = [
    0x0000, //Set/reset register (0x00)
    0x0001, //Enable set/reset register (0x01)
    0x0002, //Color compare register (0x02)
    0x0003, //Data rotate register (0x03)
    0x0004, //Read map select register (0x04)
    0x4005, //Graphics mode register (0x05)
    0x0506, //Miscellaneous Register (index 0x06)
    0x0F07, //Color don't care register (0x07)
    0xFF08  //Bit mask register (0x08)
];

pub const VGA_13H_ATTRIBUTE_CONTROLLER_REGS: [u8; 21] = [
    0x00,
    0x01,
    0x02,
    0x03,
    0x04,
    0x05,
    0x06,
    0x07,
    0x08,
    0x09,
    0x0A,
    0x0B,
    0x0C,
    0x0D,
    0x0E,
    0x0F,
    0x41,
    0x00,
    0x0F,
    0x00,
    0x00,
];
//  *MODE 0x12 640x480px 16 colors (4bit)*
pub const VGA_12H_MISC_OUTPUT_REG: u8 = 0xE3;
pub const VGA_12H_CRT_CONTROL_REGS: [u16; 18] = [
    0x5F00, //Horizontal total register (index 0x00)
    0x4F01, //Horizontal Display-Enable End Register (index 0x01)
    0x5002, //Start Horizontal Blanking Register (index 0x02)
    0x8203, //End Horizontal Blanking Register (index 0x03)
    0x5404, //Start Horizontal Retrace Pulse Register (index 0x04)
    0x8005, //End Horizontal Retrace Register (index 0x05)
    0x0B06, //Vertical Total Register (index 0x06)
    0x3E07, //Overflow Register (0x07)
    0x0008, //Preset row scan register (index 0x08)
    0x4009, //Maximum scan line regisyer (index 0x09)
    0xEA10, //Vertical Retrace Start Register (index 0x10)
    0x0C11, //Vertical Retrace End Register (index 0x11, is set first to unlock registers 0x00 to 0x07
    0xDF12, //Vertical Display-Enable End Register (0x12)
    0x2813, //Offset Register (0x13)
    0x0014, //Underline Location Register (index 0x14)
    0xE715, //Start Vertical Blanking Register (0x15)
    0x0416, //End Vertical Blanking Register (0x16)
    0xE317  //CRT Mode Control Register (0x17)
];

pub const VGA_12H_SEQUENCER_REGS: [u16; 5] = [
    0x0300, //Sequencer Address Register
    0x0101, //Clocking mode register (index 0x01)
    0x0802, //Map mask register (index 0x02)
    0x0003, //Character map select register (index 0x03)
    0x0604  //Memory mode register (index 0x04)
];

pub const VGA_12H_GRAPHICS_CONTROLLER_REGS: [u16; 9] = [
    0x0000, //Set/reset register (0x00)
    0x0001, //Enable set/reset register (0x01)
    0x0002, //Color compare register (0x02)
    0x0003, //Data rotate register (0x03)
    0x0304, //Read map select register (0x04)
    0x0005, //Graphics mode register (0x05)
    0x0506, //Miscellaneous Register (index 0x06)
    0x0F07, //Color don't care register (0x07)
    0xFF08  //Bit mask register (0x08)
];

pub const VGA_12H_ATTRIBUTE_CONTROLLER_REGS: [u8; 21] = [
    0x00,
    0x01,
    0x02,
    0x03,
    0x04,
    0x05,
    0x14,
    0x07,
    0x38,
    0x39,
    0x3A,
    0x3B,
    0x3C,
    0x3D,
    0x3E,
    0x3F,
    0x01,
    0x00,
    0x0F,
    0x00,
    0x00,
];
//--------------------------------------------------------
//  *VGA MODE 0x03 TEXT MODE 25x80 chars*
pub const VGA_03H_MISC_OUTPUT_REG: u8 = 0x67;
pub const VGA_03H_CRT_CONTROL_REGS: [u16; 18] = [
    0x5F00, //Horizontal total register (index 0x00)
    0x4F01, //Horizontal Display-Enable End Register (index 0x01)
    0x5002, //Start Horizontal Blanking Register (index 0x02)
    0x8203, //End Horizontal Blanking Register (index 0x03)
    0x5504, //Start Horizontal Retrace Pulse Register (index 0x04)
    0x8105, //End Horizontal Retrace Register (index 0x05)
    0xBF06, //Vertical Total Register (index 0x06)
    0x1F07, //Overflow Register (0x07)
    0x0008, //Preset row scan register (index 0x08)
    0x4F09, //Maximum scan line regisyer (index 0x09)
    0x9C10, //Vertical Retrace Start Register (index 0x10)
    0x8E11, //Vertical Retrace End Register (index 0x11, is set first to unlock registers 0x00 to 0x07
    0x8F12, //Vertical Display-Enable End Register (0x12)
    0x2813, //Offset Register (0x13)
    0x1F14, //Underline Location Register (index 0x14)
    0x9615, //Start Vertical Blanking Register (0x15)
    0xB916, //End Vertical Blanking Register (0x16)
    0xA317  //CRT Mode Control Register (0x17)
];

pub const VGA_03H_SEQUENCER_REGS: [u16; 5] = [
    0x0300, //Sequencer Address Register
    0x0001, //Clocking mode register (index 0x01)
    0x0302, //Map mask register (index 0x02)
    0x0003, //Character map select register (index 0x03)
    0x0204  //Memory mode register (index 0x04)
];

pub const VGA_03H_GRAPHICS_CONTROLLER_REGS: [u16; 9] = [
    0x0000, //Set/reset register (0x00)
    0x0001, //Enable set/reset register (0x01)
    0x0002, //Color compare register (0x02)
    0x0003, //Data rotate register (0x03)
    0x0004, //Read map select register (0x04)
    0x1005, //Graphics mode register (0x05)
    0x0E06, //Miscellaneous Register (index 0x06)
    0x0007, //Color don't care register (0x07)
    0x0F08  //Bit mask register (0x08)
];

pub const VGA_03H_ATTRIBUTE_CONTROLLER_REGS: [u8; 21] = [
    0x00,
    0x01,
    0x02,
    0x03,
    0x04,
    0x05,
    0x14,
    0x07,
    0x38,
    0x39,
    0x3A,
    0x3B,
    0x3C,
    0x3D,
    0x3E,
    0x3F,
    0x0C,
    0x00,
    0x0F,
    0x08,
    0x00,
];