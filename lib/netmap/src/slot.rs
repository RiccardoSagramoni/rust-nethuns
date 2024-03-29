use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::bindings::netmap_slot;

/// Safe wrapper for [`netmap_slot`] struct. It's a buffer descriptor.
#[derive(Debug)]
#[repr(transparent)]
pub struct NetmapSlot {
    netmap_slot: NonNull<netmap_slot>,
}

impl NetmapSlot {
    pub fn new(ptr: NonNull<netmap_slot>) -> Self {
        Self { netmap_slot: ptr }
    }
}

impl Deref for NetmapSlot {
    type Target = netmap_slot;
    
    fn deref(&self) -> &Self::Target {
        // [SAFETY] Safety requirements met thanks to 
        // the usage of `NonNull` to wrap the raw pointer.
        unsafe { self.netmap_slot.as_ref() }
    }
}

impl DerefMut for NetmapSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // [SAFETY] Safety requirements met thanks to 
        // the usage of `NonNull` to wrap the raw pointer.
        unsafe { self.netmap_slot.as_mut() }
    }
}
