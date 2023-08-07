use std::ops::{Deref, DerefMut};

use crate::bindings::netmap_slot;

/// Safe wrapper for `netmap_slot` struct. It's a buffer descriptor.
pub struct NetmapSlot {
    netmap_slot: *mut netmap_slot
}

impl NetmapSlot {
    pub fn try_new(ptr: *mut netmap_slot) -> Result<Self, String> {
        if ptr.is_null() {
            return Err("[NetmapSlot::try_new()] ptr is null".to_owned());
        }
        Ok(Self {
            netmap_slot: ptr,
        })
    }
}

impl Deref for NetmapSlot {
    type Target = netmap_slot;
    
    fn deref(&self) -> &Self::Target {
        assert!(!self.netmap_slot.is_null());
        unsafe { &*self.netmap_slot }
    }
}

impl DerefMut for NetmapSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.netmap_slot.is_null());
        unsafe { &mut *self.netmap_slot }
    }
}
