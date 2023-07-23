#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::{env, thread, ptr, os::raw::*};

fn meter() {
    todo!();
}

fn main() {
	// Collect and check command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        eprintln!("usage: {} in out", args[0]);
        return;
    }
    
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
		let inn: *mut nethuns_socket_netmap = nethuns_open_netmap(&mut in_opt, &mut errbuf as *mut i8);
	}
	
}
