use std::ffi::CString;

use crate::bindings::{nmport_d, nmport_open_desc, nmport_prepare};

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub struct NmPortDescriptor {
    pub d: Box<nmport_d>,
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
        
        Ok(Self {
            d: unsafe { Box::from_raw(d) },
        })
    }
    
    
    /// open an initialized port descriptor
    pub fn open_desc(&mut self) -> Result<(), String> {
        match unsafe { nmport_open_desc(self.d.as_mut()) } {
            -1 => Err(format!("{}", errno::errno())),
            0 => Ok(()),
            ret => {
                panic!("nmport_open_desc returned unexpected value {ret}");
            }
        }
    }
}

// impl Drop for NmPortDescriptor {
//     fn drop(&mut self) {
//         todo!(); // TODO NmPortDescriptor drop()
//     }
// }
