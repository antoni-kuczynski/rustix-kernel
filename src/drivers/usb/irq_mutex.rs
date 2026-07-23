/*
 * Created by Antoni Kuczyński
 * 27/07/2026
 */

//! Spin lock that keeps interrupts masked for as long as it is held.
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use spin::{Mutex, MutexGuard};
use x86_64::instructions::interrupts;

pub struct IrqMutex<T: ?Sized> {
    inner: Mutex<T>,
}

impl<T> IrqMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }

    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T: ?Sized> IrqMutex<T> {
    pub fn lock(&self) -> IrqGuard<'_, T> {
        let interrupts_were_enabled = interrupts::are_enabled();
        interrupts::disable();

        IrqGuard {
            guard: ManuallyDrop::new(self.inner.lock()),
            interrupts_were_enabled,
        }
    }

    pub fn try_lock(&self) -> Option<IrqGuard<'_, T>> {
        let interrupts_were_enabled = interrupts::are_enabled();
        interrupts::disable();

        match self.inner.try_lock() {
            Some(guard) => Some(IrqGuard {
                guard: ManuallyDrop::new(guard),
                interrupts_were_enabled,
            }),
            None => {
                if interrupts_were_enabled {
                    interrupts::enable();
                }
                None
            }
        }
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

pub struct IrqGuard<'a, T: ?Sized> {
    guard: ManuallyDrop<MutexGuard<'a, T>>,
    interrupts_were_enabled: bool,
}

impl<T: ?Sized> Deref for IrqGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T: ?Sized> DerefMut for IrqGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T: ?Sized> Drop for IrqGuard<'_, T> {
    fn drop(&mut self) {
        //first release the lock, than enable interrupts
        unsafe { ManuallyDrop::drop(&mut self.guard) };
        if self.interrupts_were_enabled {
            interrupts::enable();
        }
    }
}
