use x86_64::structures::idt::InterruptStackFrame;

pub enum XHCIInterruptIndex {
    MsiXMessageData = 0x40
}

pub extern "x86-interrupt" fn xhci_msix_irq_handler(_: InterruptStackFrame) {

}
