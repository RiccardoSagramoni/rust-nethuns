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
        unsafe { self.nmport_d.as_ref() }
    }
}

impl DerefMut for NmPortDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.nmport_d.as_mut() }
    }
}

impl Drop for NmPortDescriptor {
    /// Close the Netmap port when its descriptor is dropped.
    fn drop(&mut self) {
        unsafe { nmport_close(self.nmport_d.as_ptr()) };
    }
}
