use std::ffi::{CStr, CString};
use std::mem;
use std::os::fd::AsRawFd;

use num_traits::AsPrimitive;
use rustix::net;

use crate::global::{NethunsNetInfo, NETHUNS_GLOBAL};


/// Set interface in promiscuous mode.
///
/// # Arguments
/// * `devname`: Name of the interface/device.
///
/// # Returns
/// * `Ok(())` - If the setting was successful.
/// * `Err(String)` - If an error occurs.
pub fn __nethuns_set_if_promisc(devname: &CString) -> Result<(), String> {
    // Get the active flag word of the device.
    let mut flags = nethuns_ioctl_if(
        devname,
        IoctlRequestCode::SIOCGIFFLAGS,
        0,
    )
    .map_err(|e| {
        format!("[__nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}")
    })?;
    
    if let Ok(mut mutex_guard) = NETHUNS_GLOBAL.lock() {
        // Retrieve the global information for the interface
        let info = match mutex_guard.get_mut(devname) {
            Some(info) => info,
            None => {
                // If the information doesn't exist yet, create it
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
        
        // Increase counter of how many threads required for the promisc mode
        info.promisc_refcnt += 1;
        
        // If the interface wasn't in promisc mode,
        // set it by using `ioctl` system call
        let do_promisc = (flags & libc::IFF_PROMISC as u32) == 0;
        if do_promisc {
            flags |= libc::IFF_PROMISC as u32;
            if let Err(e) =
                nethuns_ioctl_if(devname, IoctlRequestCode::SIOCSIFFLAGS, flags)
            {
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


/// Clear the interface from the promiscuous mode. If no other thread is using
/// the interface in promiscous mode, disable the promiscuous mode for everyone.
///
/// # Arguments
/// * `devname`: Name of the interface/device.
///
/// # Returns
/// * `Ok(())` - If the setting was successful.
/// * `Err(String)` - If an error occurs.
pub fn __nethuns_clear_if_promisc(devname: &CString) -> Result<(), String> {
    // Get the active flag word of the device.
    let mut flags = nethuns_ioctl_if(
        devname,
        IoctlRequestCode::SIOCGIFFLAGS,
        0,
    )
    .map_err(|e| {
        format!("[__nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}")
    })?;
    
    
    if let Ok(mut mutex_guard) = NETHUNS_GLOBAL.lock() {
        let mut do_clear = false;
        
        // Get the global information for the interface
        // and decrement the reference counter for promisc mode
        if let Some(info) = mutex_guard.get_mut(devname) {
            info.promisc_refcnt -= 1;
            if info.promisc_refcnt <= 0 {
                do_clear = true;
            }
        }
        
        // If no other thread is using the interface in promisc mode,
        // disable the promiscuous mode by calling `ioctl`
        if do_clear {
            flags &= !(libc::IFF_PROMISC as u32);
            if let Err(e) =
                nethuns_ioctl_if(devname, IoctlRequestCode::SIOCSIFFLAGS, flags)
            {
                return Err(format!(
                    "[__nethuns_clear_if_promisc] nethuns_ioctl_if failed: {e}"
                ));
            }
            eprintln!("device {:?} promisc mode unset", devname);
        }
    };
    
    Ok(())
}


/// Call the `ioctl` system call the either get or set the current flag word
/// of the device. This would allow to set the device in promiscuous mode.
///
/// # Arguments
/// * `devname`: Name of the interface/device.
/// * `what`: The request code for the `ioctl` system call.
/// * `flags`: The new flag word to set for the device, if the request code is SIOCSIFFLAGS. Otherwise this parameter is unused.
///
/// # Returns
/// * `Ok(u32)` - If the operation was successful. If the caller passed `IoctlRequestCode::SIOCGIFFLAGS`, the return value will be the flag word of the device. Otherwise, it will be equals to the parameter `flags`.
/// * `Err(String)` - If an error occurs.
fn nethuns_ioctl_if(
    devname: &CStr,
    what: IoctlRequestCode,
    flags: u32,
) -> Result<u32, String> {
    // TODO refactor code: remove what parameter and make flags optional
    
    // Open a new socket so that we can use `ioctl`
    let socket =
        net::socket(net::AddressFamily::INET, net::SocketType::DGRAM, None)
            .map_err(|e| {
                format!("[nethuns_ioctl_if] could not open socket: {e}")
            })?;
    
    // Create a new `ifreq` object to be passed to `ioctl` system call
    // and initialize it with the device name.
    let mut ifr: libc::ifreq = unsafe { mem::zeroed() };
    devname
        .to_bytes_with_nul()
        .iter()
        .take(ifr.ifr_name.len() - 1)
        .enumerate()
        .for_each(|(i, c)| {
            ifr.ifr_name[i] = *c as i8;
        });
    
    // If the caller asked to set the flags of the device,
    // configure the `ifreq` object with the new flags
    if what == IoctlRequestCode::SIOCSIFFLAGS {
        ifr.ifr_ifru.ifru_flags = flags as i16;
    }
    
    // Call `ioctl` with the request code and the `ifreq` object.
    // If the request code is SIOSCGIFFLAGS, the system call will
    // retrieve the new flag word from the `ifreq` object.
    // If the request code is SIOCGIFFLAGS, the system call will
    // leave the current flag word in the `ifreq` object.
    let ret = unsafe { libc::ioctl(socket.as_raw_fd(), what.as_(), &ifr) };
    if ret < 0 {
        return Err(format!(
            "[nethuns_ioctl_if] ioctl({:?}, {:?}, {:?}) failed with errno {}",
            socket,
            what,
            ifr,
            errno::errno()
        ));
    }
    
    // If the caller asked to get the flags of the device,
    // returned them, otherwise return the same flags that were passed.
    Ok(if what == IoctlRequestCode::SIOCGIFFLAGS {
        unsafe { ifr.ifr_ifru.ifru_flags as u32 }
    } else {
        flags
    })
}


/// Request code for the `ioctl` system call
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
enum IoctlRequestCode {
    /// Get the active flag word of the device
    SIOCGIFFLAGS,
    /// Set the active flag word of the device
    SIOCSIFFLAGS,
}

impl AsPrimitive<u64> for IoctlRequestCode {
    fn as_(self) -> u64 {
        match self {
            Self::SIOCGIFFLAGS => libc::SIOCGIFFLAGS,
            Self::SIOCSIFFLAGS => libc::SIOCSIFFLAGS,
        }
    }
}


#[cfg(test)]
mod test {
    use super::IoctlRequestCode;
    use num_traits::AsPrimitive;
    
    #[test]
    fn test_socket_configuration_flag() {
        assert_eq!(IoctlRequestCode::SIOCGIFFLAGS.as_(), libc::SIOCGIFFLAGS);
        assert_eq!(IoctlRequestCode::SIOCSIFFLAGS.as_(), libc::SIOCSIFFLAGS);
    }
}
