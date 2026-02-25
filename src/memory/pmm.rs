use core::ptr;
use crate::boot::multiboot::MultibootInfoView;
use crate::{endKernel, vgaprintln};
use crate::memory::kernel_end;
use crate::memory::pmm::PmmInitError::NoMemorySizeProvided;

#[derive(Debug)]
pub enum PmmInitError {
    NoMemorySizeProvided = 1
}

pub fn init(multiboot_info: &MultibootInfoView) -> Result<(), PmmInitError> {
    //TODO
    Ok(())
}