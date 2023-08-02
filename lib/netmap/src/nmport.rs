use std::{ffi::CString, ops::{DerefMut, Deref}};

use crate::bindings::{nmport_d, nmport_open_desc, nmport_prepare, nmport_close};

#[derive(Debug)]
pub struct NmPortDescriptor {
    pub d: *mut nmport_d,
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
        
        Ok(Self { d })
    }
    
    
    /// open an initialized port descriptor
    pub fn open_desc(&mut self) -> Result<(), String> {
        match unsafe { nmport_open_desc(self.d) } {
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
        assert!(!self.d.is_null());
        unsafe { &*self.d }
    }
}

impl DerefMut for NmPortDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.d.is_null());
        unsafe { &mut *self.d }
    }
}

impl Drop for NmPortDescriptor {
    fn drop(&mut self) {
        if !self.d.is_null() {
            unsafe { nmport_close(self.d) };
        }
    }
}
