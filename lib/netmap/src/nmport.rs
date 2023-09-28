use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::bindings::{
    nmport_close, nmport_d, nmport_open_desc, nmport_prepare,
};

/// Safe wrapper for nmport_d. It describes a netmap port.
#[derive(Debug)]
pub struct NmPortDescriptor {
    nmport_d: NonNull<nmport_d>,
}


impl NmPortDescriptor {
    /// Create a port descriptor, but do not open it.
    ///
    /// Equivalent to `nmport_prepare(portspec.as_ptr())`
    pub fn prepare(portspec: CString) -> Result<Self, String> {
        // [SAFETY] ok: portspec is a CString, thus it can be safely passed to C functions
        let nmport_d =
            NonNull::new(unsafe { nmport_prepare(portspec.as_ptr()) }).ok_or(
                format!("nmport_prepare(portspec = {:?}) failed", portspec),
            )?;
        
        Ok(Self { nmport_d })
    }
    
    
    /// Open an initialized port descriptor.
    ///
    /// Equivalent to `nmport_open_desc(self.nmport_d)`
    pub fn open_desc(&mut self) -> Result<(), String> {
        // [SAFETY] ok: self.nmport_d is a guaranteed to be non-null
        match unsafe { nmport_open_desc(self.nmport_d.as_ptr()) } {
            -1 => Err(format!("{}", errno::errno())),
            0 => Ok(()),
            ret => {
                panic!("nmport_open_desc returned unexpected value {ret}");
            }
        }
    }
}

impl Deref for NmPortDescriptor {
    type Target = nmport_d;
    
    fn deref(&self) -> &Self::Target {
        // [SAFETY] Safe thanks to NonNull wrapper
        unsafe { self.nmport_d.as_ref() }
    }
}

impl DerefMut for NmPortDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // [SAFETY] Safe thanks to NonNull wrapper
        unsafe { self.nmport_d.as_mut() }
    }
}

impl Drop for NmPortDescriptor {
    /// Close the Netmap port when its descriptor is dropped.
    fn drop(&mut self) {
        // [SAFETY] nmport_d is guaranteed to be non-null
        unsafe { nmport_close(self.nmport_d.as_ptr()) };
    }
}
