use std::ffi::{CStr, CString};
use std::mem;
use std::sync::Mutex;

use num_traits::AsPrimitive;

use crate::global::{NethunsNetInfo, NETHUNS_GLOBAL};
use crate::NethunsSocket;


///
pub fn __nethuns_set_if_promisc(
    s: &impl NethunsSocket,
    devname: &CString,
) -> Result<(), String> {
    let mut flags =
        nethuns_ioctl_if(s, devname, SocketConfigurationFlag::SIOCGIFFLAGS, 0)
            .map_err(|e| {
                format!(
                    "[__nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}"
                )
            })?;
    
    if let Ok(mut mutex_guard) = NETHUNS_GLOBAL.lock() {
        let info = match mutex_guard.get_mut(devname) {
            Some(info) => info,
            None => {
                mutex_guard
                    .insert(devname.to_owned(), NethunsNetInfo::default());
                let info = mutex_guard.get_mut(devname).expect(&format!(
                    "failed to get info for device {:?}",
                    devname
                ));
                info.promisc_refcnt = if (flags & libc::IFF_PROMISC as u32) == 0
                {
                    0
                } else {
                    1
                };
                info
            }
        };
        
        info.promisc_refcnt += 1;
        
        let do_promisc = (flags & libc::IFF_PROMISC as u32) == 0;
        
        if do_promisc {
            flags |= libc::IFF_PROMISC as u32;
            if let Err(e) = nethuns_ioctl_if(
                s,
                devname,
                SocketConfigurationFlag::SIOCSIFFLAGS,
                flags,
            ) {
                info.promisc_refcnt -= 1;
                return Err(format!(
                    "[__nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}"
                ))
            }
        }
        
        if do_promisc {
            eprintln!("device {:?} promisc mode set", devname);
        }
        else {
            eprintln!("device {:?} (already) promisc mode set", devname);
        }
    };
    
    Ok(())
}


fn nethuns_ioctl_if(
    s: &impl NethunsSocket,
    devname: &CStr,
    what: SocketConfigurationFlag,
    flags: u32,
) -> Result<u32, String> {
    let x = what as u64;
    todo!()
}

#[derive(Clone, Copy, Debug)]
enum SocketConfigurationFlag {
    SIOCGIFFLAGS,
    SIOCSIFFLAGS,
}

impl AsPrimitive<u64> for SocketConfigurationFlag {
    fn as_(self) -> u64 {
        match self {
            Self::SIOCGIFFLAGS => libc::SIOCGIFFLAGS,
            Self::SIOCSIFFLAGS => libc::SIOCSIFFLAGS,
        }
    }
}
