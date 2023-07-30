use std::{ffi::CString, ops::Deref};

use crate::bindings::{nmport_d, nmport_open_desc, nmport_prepare};

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub struct NmPortDescriptor {
    pub d: Box<nmport_d>,
}

impl NmPortDescriptor {
    /// Create a port descriptor, but do not open it.
    ///
    /// Equivalent to `nmport_prepare(portspec.as_ptr())`
    pub fn prepare(portspec: CString) -> Result<NmPortDescriptor, String> {
        let d = unsafe { nmport_prepare(portspec.as_ptr()) };
        if d.is_null() {
            return Err(format!(
                "nmport_prepare(portspec = {:?}) failed",
                portspec
            )
            .to_owned());
        }
        
        return Ok(NmPortDescriptor {
            d: unsafe { Box::from_raw(d) },
        });
    }
    

    /// open an initialized port descriptor
    pub fn open_desc(&mut self) -> Result<(), String> {
        match unsafe { nmport_open_desc(&mut *self.d as *mut nmport_d) } {
            -1 => {
                return Err(format!("{}", errno::errno()));
            }
            0 => {
                return Ok(());
            }
            ret => {
                panic!("nmport_open_desc returned unexpected value {ret}");
            }
        }
    }
}

impl Drop for NmPortDescriptor {
    fn drop(&mut self) {
        todo!(); // TODO NmPortDescriptor drop()
    }
}
