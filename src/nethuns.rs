use std::ffi::{CStr, CString};
use std::mem;
use std::os::fd::AsRawFd;

use num_traits::AsPrimitive;
use rustix::net;

use crate::global::{NethunsNetInfo, NETHUNS_GLOBAL};


///
pub fn __nethuns_set_if_promisc(devname: &CString) -> Result<(), String> {
    let mut flags =
        nethuns_ioctl_if(devname, SocketConfigurationFlag::SIOCGIFFLAGS, 0)
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
                let info = mutex_guard.get_mut(devname).ok_or(format!(
                    "failed to get info for device {:?}",
                    devname
                ))?;
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
                devname,
                SocketConfigurationFlag::SIOCSIFFLAGS,
                flags,
            ) {
                info.promisc_refcnt -= 1;
                return Err(format!(
                    "[__nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}"
                ));
            }
        }
        
        if do_promisc {
            eprintln!("device {:?} promisc mode set", devname);
        } else {
            eprintln!("device {:?} (already) promisc mode set", devname);
        }
    };
    
    Ok(())
}


///
fn nethuns_ioctl_if(
    devname: &CStr,
    what: SocketConfigurationFlag,
    flags: u32,
) -> Result<u32, String> {
    let socket =
        net::socket(net::AddressFamily::INET, net::SocketType::DGRAM, None)
            .map_err(|e| {
                format!("[nethuns_ioctl_if] could not open socket: {e}")
            })?;
    
    let mut ifr: libc::ifreq = unsafe { mem::zeroed() };
    devname
        .to_bytes_with_nul()
        .iter()
        .take(ifr.ifr_name.len() - 1)
        .enumerate()
        .for_each(|(i, c)| {
            ifr.ifr_name[i] = *c as i8;
        });
    
    if what == SocketConfigurationFlag::SIOCSIFFLAGS {
        ifr.ifr_ifru.ifru_flags = flags as i16;
    }
    
    let ret = unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            what as u64,
            &ifr as *const libc::ifreq as *const libc::c_void,
        )
    };
    if ret < 0 {
        // FIXME nethuns_perror(nethuns_socket(s)->errbuf, "ioctl");
        return Err(format!(
            "[nethuns_ioctl_if] ioctl({:?}, {:?}, {:?}) failed",
            socket, what, ifr
        ));
    }
    
    Ok(if what == SocketConfigurationFlag::SIOCGIFFLAGS {
        unsafe { ifr.ifr_ifru.ifru_flags as u32 }
    } else {
        flags
    })
}


#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
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
