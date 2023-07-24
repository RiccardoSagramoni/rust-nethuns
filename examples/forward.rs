#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use core::ffi::c_char;
use std::env;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;
use std::thread;

struct Configuration {
    dev_in: CString,
    dev_out: CString,
}

fn meter() {
    todo!();
}

fn generate_configuration(args: Vec<String>) -> Configuration {
    if args.len() < 3 {
        panic!("usage: {} in out", args[0]);
    }
    
    return Configuration {
        dev_in: CString::new(args[1]).expect("Unable to parse args[1]"),
        dev_out: CString::new(args[2]).expect("Unable to parse args[1]"),
    };
}

fn main() {
    // Collect and parse command line arguments
    let conf = generate_configuration(env::args().collect());
    
    let thread = thread::spawn(meter);
    
    let mut in_opt = nethuns_socket_options {
        numblocks: 4,
        numpackets: 65536,
        packetsize: 2048,
        timeout_ms: 20,
        dir: nethuns_capture_dir_nethuns_in_out,
        capture: nethuns_capture_mode_nethuns_cap_default,
        mode: nethuns_socket_mode_nethuns_socket_rx_tx,
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        xdp_prog: ptr::null(),
        xdp_prog_sec: ptr::null(),
        xsk_map_name: ptr::null(),
        reuse_maps: false,
        pin_dir: ptr::null(),
    };
    
    let mut out_opt = nethuns_socket_options {
        numblocks: 4,
        numpackets: 65536,
        packetsize: 2048,
        timeout_ms: 20,
        dir: nethuns_capture_dir_nethuns_in_out,
        capture: nethuns_capture_mode_nethuns_cap_default,
        mode: nethuns_socket_mode_nethuns_socket_rx_tx,
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        xdp_prog: ptr::null(),
        xdp_prog_sec: ptr::null(),
        xsk_map_name: ptr::null(),
        reuse_maps: false,
        pin_dir: ptr::null(),
    };
    
    let mut errbuf: [c_char; NETHUNS_ERRBUF_SIZE as usize] = [0; NETHUNS_ERRBUF_SIZE as usize];
    
    unsafe {
        let socket_in = nethuns_open_netmap(&mut in_opt, &mut errbuf as *mut i8);
        assert!(
            socket_in.is_null() == false,
            "{}",
            CStr::from_ptr(errbuf.as_ptr()).to_string_lossy()
        );
        
        let socket_out = nethuns_open_netmap(&mut out_opt, errbuf.as_mut_ptr());
        assert!(
            socket_out.is_null() == false,
            "{}",
            CStr::from_ptr(errbuf.as_ptr()).to_string_lossy()
        );
        
        let result = nethuns_bind_netmap(socket_in, conf.dev_in.as_ptr(), NETHUNS_ANY_QUEUE);
        if (result )
        
    }
}
