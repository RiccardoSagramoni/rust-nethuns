#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use core::ffi::c_char;
use std::env;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Debug)]
struct Configuration {
    dev_in: CString,
    dev_out: CString,
}

#[derive(Debug)]
enum ForwardError {
    NethunsException(*mut nethuns_socket_netmap),
    Exception(String),
}

fn main() {
    // Collect and parse command line arguments
    let conf = generate_configuration(env::args().collect());
    dbg!(&conf);
    
    // Generate options for socket
    let (in_opt, out_opt) = generate_socket_opt();
    dbg!(&in_opt, &out_opt);
    
    // Run example
    if let Err(error) = run_forward(conf, in_opt, out_opt) {
        match error {
            ForwardError::NethunsException(socket) => unsafe {
                nethuns_close_netmap(socket);
                eprintln!("nethuns socket error: {:#?}", socket);
            },
            ForwardError::Exception(message) => {
                eprintln!("{}", message);
            }
        }
    }
}

fn generate_configuration(args: Vec<String>) -> Configuration {
    if args.len() < 3 {
        panic!("usage: {} in out", args[0]);
    }
    
    return Configuration {
        dev_in: CString::new(args[1].as_str()).expect("Unable to parse args[1]"),
        dev_out: CString::new(args[2].as_str()).expect("Unable to parse args[1]"),
    };
}

fn generate_socket_opt() -> (nethuns_socket_options, nethuns_socket_options) {
    let in_opt = nethuns_socket_options {
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
    
    let out_opt = nethuns_socket_options {
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
    
    (in_opt, out_opt)
}

fn run_forward(
    conf: Configuration,
    mut in_opt: nethuns_socket_options,
    mut out_opt: nethuns_socket_options,
) -> Result<(), ForwardError> {
    // Allocate error buffer for C functions
    let mut errbuf: [c_char; NETHUNS_ERRBUF_SIZE as usize] = [0; NETHUNS_ERRBUF_SIZE as usize];
    
    // Open input socket
    let socket_in: *mut nethuns_socket_netmap =
        unsafe { nethuns_open_netmap(&mut in_opt, errbuf.as_mut_ptr()) };
    if socket_in.is_null() {
        return Err(ForwardError::Exception(unsafe {
            CString::from_raw(errbuf.as_mut_ptr())
                .into_string()
                .expect("into_string() call failed")
        }));
    }
    
    // Open output socket
    let socket_out: *mut nethuns_socket_netmap =
        unsafe { nethuns_open_netmap(&mut out_opt, errbuf.as_mut_ptr()) };
    if socket_out.is_null() {
        return Err(ForwardError::Exception(unsafe {
            CString::from_raw(errbuf.as_mut_ptr())
                .into_string()
                .expect("into_string() call failed")
        }));
    }
    
    // Bind input socket
    let result: i32 =
        unsafe { nethuns_bind_netmap(socket_in, conf.dev_in.as_ptr(), NETHUNS_ANY_QUEUE) };
    if result < 0 {
        return Err(ForwardError::NethunsException(socket_in));
    }
    
    // Bind output socket
    let result: i32 =
        unsafe { nethuns_bind_netmap(socket_out, conf.dev_out.as_ptr(), NETHUNS_ANY_QUEUE) };
    if result < 0 {
        return Err(ForwardError::NethunsException(socket_out));
    }
    
    // Spawn thread for monitoring received and forwarded packets
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));
    let total_rcv_clone = total_rcv.clone();
    let total_fwd_clone = total_fwd.clone();
    let _thread = thread::spawn(move || {
        meter(total_rcv_clone.clone(), total_fwd_clone.clone());
    });
    
    let frame: *mut *const u8 = ptr::null_mut();
    let pkthdr: *mut *const netmap_pkthdr = ptr::null_mut();
    
    loop {
        let pkt_id = unsafe { nethuns_recv_netmap(socket_in, pkthdr, frame) };
        
        if pkt_id == 0 {
            continue;
        }
        
        total_rcv.fetch_add(1, Ordering::SeqCst);
        
        unsafe {
            while 0 != nethuns_send_netmap(socket_out, *frame, (*(*pkthdr)).len) {
                nethuns_flush_netmap(socket_out);
            }
        }
        
        total_fwd.fetch_add(1, Ordering::SeqCst);
        
        unsafe {
            // nethuns_rx_release(socket_in, pkt_id);
            todo!();
        }
        break;
    }
    
    todo!();
}

///
fn meter(total_rcv: Arc<AtomicU64>, total_fwd: Arc<AtomicU64>) {
    let mut now = SystemTime::now();
    
    loop {
        // Sleep for 1 second
        let next_sys_time = now
            .checked_add(Duration::from_secs(1))
            .expect("SystemTime::checked_add() failed");
        if let Ok(delay) = next_sys_time.duration_since(now) {
            thread::sleep(delay);
        }
        now = next_sys_time;
        
        // Print current counters
        let r = total_rcv.swap(0, Ordering::SeqCst);
        let f = total_fwd.swap(0, Ordering::SeqCst);
        println!("pkt/sec: {r} fwd/s {f}");
    }
}
