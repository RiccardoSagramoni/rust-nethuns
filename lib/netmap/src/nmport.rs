use std::{ffi::CString, ops::{DerefMut, Deref}};

use crate::bindings::{nmport_d, nmport_open_desc, nmport_prepare, nmport_close};

/// Safe wrapper for nmport_d. It describes a netmap port.
#[derive(Debug)]
pub struct NmPortDescriptor {
    nmport_d: *mut nmport_d,
}

impl NmPortDescriptor {
    /// Create a port descriptor, but do not open it.
    ///
    /// Equivalent to `nmport_prepare(portspec.as_ptr())`
    pub fn prepare(portspec: CString) -> Result<Self, String> {
        let d = unsafe { nmport_prepare(portspec.as_ptr()) };
        if d.is_null() {
            return Err(format!(
                "nmport_prepare(portspec = {:?}) failed",
                portspec
            ));
        }
        
        Ok(Self { nmport_d: d })
    }
    
    
    /// Open an initialized port descriptor.
    /// 
    /// Equivalent to `nmport_open_desc(self.nmport_d)`
    pub fn open_desc(&mut self) -> Result<(), String> {
        assert!(!self.nmport_d.is_null());
        match unsafe { nmport_open_desc(self.nmport_d) } {
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
        assert!(!self.nmport_d.is_null());
        unsafe { &*self.nmport_d }
    }
}

impl DerefMut for NmPortDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.nmport_d.is_null());
        unsafe { &mut *self.nmport_d }
    }
}

impl Drop for NmPortDescriptor {
    /// Close the Netmap port when its descriptor is dropped.
    fn drop(&mut self) {
        if !self.nmport_d.is_null() {
            unsafe { nmport_close(self.nmport_d) };
        }
    }
}
