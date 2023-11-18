//! Miscellaneous and utility functions.

pub(crate) mod circular_buffer;

use std::ffi::CStr;
use std::mem;

use rustix::fd::AsRawFd;
use rustix::net;

use crate::global::{NethunsNetInfo, NETHUNS_GLOBAL};
use crate::sockets::NethunsSocket;
use crate::types::NethunsQueue;


/// Get full device name, taking into account
/// both the real device name and the queue.
#[inline(always)]
pub fn nethuns_dev_queue_name(
    dev: Option<&str>,
    queue: NethunsQueue,
) -> String {
    match dev {
        None => "unspec".to_owned(),
        Some(dev) => match queue {
            NethunsQueue::Some(idx) => {
                format!("{}:{}", dev, idx)
            }
            NethunsQueue::Any => dev.to_owned(),
        },
    }
}


/// Get the name of the device bounded to the socket.
#[inline(always)]
pub fn nethuns_device_name(socket: &NethunsSocket) -> String {
    nethuns_dev_queue_name(
        socket.base().devname().to_str().ok(),
        socket.base().get_queue(),
    )
}


/// Set interface in promiscuous mode.
///
/// # Arguments
/// * `devname`: Name of the interface/device.
///
/// # Returns
/// * `Ok(())` - If the setting was successful.
/// * `Err(String)` - If an error occurs.
pub(crate) fn nethuns_set_if_promisc(devname: &CStr) -> Result<(), String> {
    // Get the active flag word of the device.
    let mut flags = nethuns_ioctl_if(devname, None)
        .map_err(|e| {
            format!("[nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}")
        })?
        .expect("Unexpected None value for flags");
    
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
            if let Err(e) = nethuns_ioctl_if(devname, Some(flags)) {
                info.promisc_refcnt -= 1;
                return Err(format!(
                    "[nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}"
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
pub(crate) fn nethuns_clear_if_promisc(devname: &CStr) -> Result<(), String> {
    // Get the active flag word of the device.
    let mut flags = nethuns_ioctl_if(devname, None)
        .map_err(|e| {
            format!("[nethuns_set_if_promisc] nethuns_ioctl_if failed: {e}")
        })?
        .expect("Unexpected None value for flags");
    
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
            if let Err(e) = nethuns_ioctl_if(devname, Some(flags)) {
                return Err(format!(
                    "[nethuns_clear_if_promisc] nethuns_ioctl_if failed: {e}"
                ));
            }
            eprintln!("device {:?} promisc mode unset", devname);
        }
    };
    
    Ok(())
}


/// Call the `ioctl` system call the either get or set the current flag word
/// of the device.
///
/// This function allows to set the device in promiscuous mode.
///
/// # Arguments
/// * `devname`: Name of the interface/device.
/// * `flags`: If you want to **get** the current flag word from the interface, pass `None`. If you want to **set** the flag word, pass `Some(flag)`.
///
/// # Returns
/// * `Ok(Some(u32))` - If the operation was successful and the caller passed `None` as `flags` parameter.
/// * `Ok(None)` - If the operation was successful and the caller passed `Some(u32)` as `flags` parameter.
/// * `Err(String)` - If an error occurs.
fn nethuns_ioctl_if(
    devname: &CStr,
    flags: Option<u32>,
) -> Result<Option<u32>, String> {
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
            ifr.ifr_name[i] = *c as _;
        });
    
    // If the caller asked to set the flags of the device,
    // configure the `ifreq` object with the new flags
    if let Some(flags) = flags {
        ifr.ifr_ifru.ifru_flags = flags as _;
    }
    
    // Select request code for the required operation
    let request_code = match flags {
        // Set the active flag word of the device
        Some(_) => libc::SIOCSIFFLAGS,
        // Get the active flag word of the device
        None => libc::SIOCGIFFLAGS,
    };
    
    // Call `ioctl` with the request code and the `ifreq` object.
    // If the request code is SIOSCGIFFLAGS, the system call will
    // retrieve the new flag word from the `ifreq` object.
    // If the request code is SIOCGIFFLAGS, the system call will
    // leave the current flag word in the `ifreq` object.
    let ret = unsafe { libc::ioctl(socket.as_raw_fd(), request_code, &ifr) };
    if ret < 0 {
        return Err(format!(
            "[nethuns_ioctl_if] ioctl({:?}, {}, {:?}) failed with errno {}",
            socket,
            request_code,
            ifr,
            errno::errno()
        ));
    }
    
    // If the caller asked to get the flags of the device,
    // returned them, otherwise return the same flags that were passed.
    Ok(match flags {
        Some(_) => None,
        None => Some(unsafe { ifr.ifr_ifru.ifru_flags } as u32),
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_nethuns_dev_queue_name() {
        assert_eq!(
            nethuns_dev_queue_name(None, NethunsQueue::Some(123)),
            "unspec".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(None, NethunsQueue::Any),
            "unspec".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(Some("eth0"), NethunsQueue::Some(123)),
            "eth0:123".to_owned(),
        );
        assert_eq!(
            nethuns_dev_queue_name(Some("eth0"), NethunsQueue::Any),
            "eth0".to_owned(),
        );
    }
}
