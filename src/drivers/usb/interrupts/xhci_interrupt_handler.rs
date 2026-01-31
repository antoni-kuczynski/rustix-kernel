use x86_64::structures::idt::InterruptStackFrame;

pub enum XHCIInterruptIndex {
    MsiXCommandPortData = 0x40,
    MsiXTransferEvents = 0x41
}

pub extern "x86-interrupt" fn xhci_msix_command_data_irq_handler(_: InterruptStackFrame) {

}

pub extern "x86-interrupt" fn xhci_msix_transfer_irq_handler(_: InterruptStackFrame) {

}
