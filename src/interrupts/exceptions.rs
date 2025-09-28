use core::sync::atomic::{AtomicU8, Ordering};

use x86_64::{instructions::hlt, registers::control::Cr2, structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode}};

use crate::{vgaprintln};

/*
 * Created by Oskar Przybylski
 * 22/09/2025
 *
 * List of cpu exceptions can be found here : https://wiki.osdev.org/Exceptions
 * To handle cpu exceptions we have to set
 * IDT (Interrupt Descriptor Table) structure.
 * (Note: Some of the exceptions push Error code (32 bit in real mode, 64 bit in long mode)
 *  to the stack, this value MUST be pulled from the stack before returning)
 * Each exception has a predefined IDT index as following:
 *  Name                    IDT index   Type        Mnemonic    Error code?         Implemented
 *  Division Error              0  (0x0)     Fault       #DE         No             Yes
 *  Debug                       1  (0x1)     Fault/Trap  #DB         No
 *  Non-maskable Interrupt      2  (0x2)     Interrupt   -           No
 *  Breakpoint                  3  (0x3)     Trap        #BP         No             Yes 
 *  Overflow                    4  (0x4)     Trap        #OF         No
 *  Bound Range Exceeded        5  (0x5)     Fault       #BR         No
 *  Invalid Optcode             6  (0x6)     Fault       #UD         No             Yes 
 *  Device Not Available        7  (0x7)     Fault       #NM         No
 *  Double Fault                8  (0x8)     Abort       #DF         Yes (Zero)     Yes 
 *  Reserved                    9  (0x9)     Fault       -           No
 *  Invalid TSS                 10 (0xA)     Fault       #TS         Yes
 *  Segment Not Present         11 (0xB)     Fault       #NP         Yes
 *  Stack-Segment Fault         12 (0xC)     Fault       #SS         Yes
 *  General Protection Fault    13 (0xD)     Fault       #GP         Yes            Yes 
 *  Page Fault                  14 (0xE)     Fault       #PF         Yes            Yes 
 *  Reserved                    15 (0xF)     -           -           No
 *  x87 FP Exception            16 (0x10)    Fault       #MF         No
 *  Alignment Check             17 (0x11)    Fault       #AC         Yes
 *  Machine Check               18 (0x12)    Abort       #MC         No
 *  SIMD FP Exception           19 (0x13)    Fault       #XM/#XF     No
 *  Virt Exception              20 (0x14)    Fault       #VE         No
 *  Control Protection Excp     21 (0x15)    Fault       #Cp         Yes
 *  Reserved                    22-27 (0x16-0x1B) -      -           No
 *  Hypervisor Injc Excp        28 (0x1C)    Fault       #HV         No
 *  VMM Comm Excp               29 (0x1D)    Fault       #VC         Yes
 *  Seurity Excp                30 (0x1F)    Fault       #SX         Yes
 *  Reserved                    31 (0x1f)    -           -           No
 *  Triple Fault                -            -           -           No
 *  Reserved                    IQR 13       Interrupt   #FERR       No
 *
 * TODO: implement handlers for all this exceptions ^
 *
 * The hardware enforces following format for the IDT.
 * We use Entry<F> from x86_64 crate 
 * Each entry must follow this 16-byte structure:
 *  Type    Name                    Description
 *  u16     Function Pointer[0:15]  The lower bits of the pointer to the handler function.
 *  u16     GDT selector            Selector of code segment in the global descriptor table.
 *  u16     Options                 See below.
 *  u16     Function Pointer[16:31] The middle bits of the pointer to the handler functions.
 *  u32     Function Pointer[32:63] Remaining bits of the pointer to the handler function.
 *  u32     Reserved                --
 *
 *  Where options has following format:
 *  Bits    Name                            Description
 *  0-2     Interrupt Stack Table Index     0 - dont switch stack, 1-7 - switch to the n-th stack in
 *                                          the Interrupt Stack Table when this is called.
 *  3-7     Reserved                        --
 *  8       Gate                            0 - Interrupt gate (interrputs are disabled when this
 *                                          is called) , 1 - Trap gate.
 *  9-11    Ones                            Always must be Ones
 *  12      Zero                            Always Zero
 *  13-14   DPL                             Descriptor Privilage Level - The minimal privilage
 *                                          level required for calling this handler.
 *  15      Present                         --
 *
 */

static LAST_EXCEPTION: AtomicU8 = AtomicU8::new(0);


/* thanks to x86_64 we do not have to worry about calling convention */
// this handler is invoked when x86_64 int3 is called
pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame){
    LAST_EXCEPTION.store(3,Ordering::SeqCst);
    vgaprintln!("EXCEPTION: BREAKPOINT: \n {:#?}",stack_frame);
}

/* double fault handler can be invoked with this odly specific
* combinations of exceptions:
*
* First Exception               Second Exception
* ========================================================
* Divide-by-zero,            |   Invalid Tss,
* Invalid TSS,               |   Segment Not Present,
* Segment Not Present,       |   Stack-Seg Fault,
* Stack-Seg Fault,           |   General Protection Fault
* General Protection Fault   |
* --------------------------------------------------------
* Page Fault                 |   Page Fault,
*                            |   Invalid TSS,
*                            |   Segment Not Present,
*                            |   Stack-Seg Fault,
*                            |   General Protection Fault
* ========================================================
*
* for example, a divide-by-zero fault followed by a page fault is fine
* (the page fault handler is invoked),but a divide-by-zero fault
* followed by a general-protection fault leads to a double fault.
*/
pub extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> !{
    vgaprintln!("LAST_EXCEPTION: {:?}",LAST_EXCEPTION);
    vgaprintln!("EXCEPTION: DOUBLE FAULT (_e:{}): \n {:?}",_error_code,stack_frame);
    panic!("Double fault occured");
}

pub extern "x86-interrupt" fn invalid_optcode_handler(stack_frame: InterruptStackFrame ){
    LAST_EXCEPTION.store(6, Ordering::SeqCst);
    vgaprintln!("EXCEPTION: INVALID OPTCODE: \n {:?}",stack_frame);
    loop{ hlt(); }
}

pub extern "x86-interrupt" fn general_protection_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64){
    LAST_EXCEPTION.store(13, Ordering::SeqCst);
    vgaprintln!("EXCEPTION: GENERAL PROTECTION FAULT (_e:{}): \n {:?}",_error_code,stack_frame);
    loop{ hlt(); }
}

pub extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, _error_code: PageFaultErrorCode){
    LAST_EXCEPTION.store(14, Ordering::SeqCst);
    vgaprintln!("EXCEPTION: PAGE FAULT (_e:{:#?}): \n {:?}",_error_code,stack_frame);
    vgaprintln!("CR2: {:?}",Cr2::read());
    loop{ hlt(); }
}

pub extern "x86-interrupt" fn division_error_handler(stack_frame: InterruptStackFrame ){
    LAST_EXCEPTION.store(0, Ordering::SeqCst);
    vgaprintln!("EXCEPTION: DIVISION ERROR: \n {:?}",stack_frame);
    loop{ hlt(); }
}
