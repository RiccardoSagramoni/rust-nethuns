#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use bus::{Bus, BusReader};
use pico_args;
use std::collections::VecDeque;
use std::error::Error;
use std::ffi::{c_char, c_int, c_long, c_uchar, CStr, CString};
use std::os::raw::c_void;
use std::{env, ptr, thread};

const HELP_BRIEF: &str = "\
Usage:  nethuns-send [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -b <batch_sz> ]   set batch size
            [ -n <nsock> ]      set number of sockets
            [ -m ]              enable multithreading
            [ -z ]              enable send zero-copy
";

const HELP_LONG: &str = "\
Usage:  nethuns-send [ options ]

-h, --help                      Show program usage and exit

Required options:

-i, --interface     <ifname>    Name of the network interface that nethuns-send operates on.

Other options:

-b, --batch_size    <batch_sz>  Batch size for packet transmission (default = 1).

-n, --sockets       <nsock>     Number of sockets to use. By default, only one socket is used.

-m, --multithreading            Enable multithreading. By default, only one thread is used.
                                If multithreading is enabled, and there is more than one socket in use,
                                each socket is handled by a separated thread.

-z, --zerocopy                  Enable send zero-copy. By default, classic send that requires a copy is used.
";

#[derive(Debug)]
struct Args {
    interface: CString,
    batch_size: c_int,
    num_sockets: c_int,
    multithreading: bool,
    zerocopy: bool,
}

#[derive(Debug)]
enum SendError {
    NethunsException(*mut nethuns_socket_netmap),
    Exception(String),
}

fn main() {
    // Parse options from command line
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error in parsing command line options: {e}.");
            eprint!("{}", HELP_BRIEF);
            std::process::exit(0);
        }
    };
    
    println!(
        "Test {} started with parameters: \n{:#?}",
        env::args().next().unwrap(),
        args
    );
    
    // Define payload for packets
    let payload: [c_uchar; 34] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
        0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
        0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
        0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
        0x07, 0x08,
    ];
    
    // Nethuns options
    let mut netopt: nethuns_socket_options = nethuns_socket_options {
        numblocks: 1,
        numpackets: 2048,
        packetsize: 2048,
        timeout_ms: 0,
        dir: nethuns_capture_dir_nethuns_in_out,
        capture: nethuns_capture_mode_nethuns_cap_zero_copy,
        mode: nethuns_socket_mode_nethuns_socket_rx_tx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: true,
        xdp_prog: ptr::null(),
        xdp_prog_sec: ptr::null(),
        xsk_map_name: ptr::null(),
        reuse_maps: false,
        pin_dir: ptr::null(),
    };
    
    // output sockets
    // let mut out_sockets: Vec<*mut nethuns_socket_t> =
    //     vec![ptr::null_mut(); args.num_sockets as usize];
    
    // one errbuf per thread
    // let mut errbuf: Vec<[c_char; NETHUNS_ERRBUF_SIZE as usize]> =
    //     vec![[0; NETHUNS_ERRBUF_SIZE as usize]; args.num_sockets as usize];
    
    // stats counter
    // TODO: sincronizzazione
    let totals: Vec<c_long> = vec![0; args.num_sockets as usize];
    
    // Create a thread for computing statistics
    let stats_th = thread::spawn(meter);
    // Vector of threads
    let mut threads: VecDeque<thread::JoinHandle<()>> = VecDeque::new();
    // Define bus for SPMC communication between threads
    let mut bus: Bus<()> = Bus::new(5);
    
    // case single thread (main) with generic number of sockets
    if !args.multithreading {
        // Define bus reader for receiving SIGINT signal
        let rx = bus.add_rx();
        set_sigint_handler(bus);
        
        // Vector for storing socket ids
        let mut out_sockets: Vec<*mut nethuns_socket_t> =
            vec![ptr::null_mut(); args.num_sockets as usize];
        
        if let Err(e) = st_send(&args, &mut netopt, &mut out_sockets, &payload, rx) {
            match e {
                SendError::NethunsException(s) => {
                    if s.is_null() == false {
                        unsafe {
                            nethuns_close_netmap(s);
                        }
                    }
                    eprintln!("Nethuns socket failed: {:?}", s);
                }
                SendError::Exception(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
        
        // Close all sockets
        for s in out_sockets {
            if s.is_null() == false {
                unsafe {
                    nethuns_close_netmap(s);
                }
            }
        }
        
    } else {
        // case multithreading enabled (num_threads == num_sockets)
        for _ in 0..args.num_sockets {
            threads.push_back(thread::spawn(mt_send));
        }
        set_sigint_handler(bus);
    }
    
    for t in threads {
        if let Err(e) = t.join() {
            eprintln!("Error joining thread: {:?}", e);
        }
    }
    
    // Wait for the threads to finish and close the sockets
    // TODO
    // for socket in out_sockets {
    //     if args.multithreading {
    //         if let Some(t) = threads.pop_front() {
    //             if let Err(e) = t.join() {
    //                 eprintln!("Error joining thread: {:?}", e);
    //             }
    //         }
    //     }
    
    //     // TODO
    //     // if socket.is_null() == false {
    //     //     unsafe {
    //     //         nethuns_close_netmap(socket);
    //     //     }
    //     // }
    // }
    
    if let Err(e) = stats_th.join() {
        eprintln!("Error joining stats thread: {:?}", e);
    }
}

///
fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut pargs = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP_LONG);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: CString::new::<String>(pargs.value_from_str(["-i", "--interface"])?)?,
        batch_size: pargs.value_from_str(["-b", "--batch_size"]).unwrap_or(1),
        num_sockets: pargs.value_from_str(["-n", "--sockets"]).unwrap_or(1),
        multithreading: pargs.contains(["-m", "--multithreading"]),
        zerocopy: pargs.contains(["-z", "--zerocopy"]),
    };
    
    // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }
    
    Ok(args)
}

///
fn char_array_to_string(arr: &[c_char]) -> String {
    unsafe {
        return CStr::from_ptr(arr.as_ptr())
            .to_owned()
            .into_string()
            .expect("into_string() call failed");
    }
}

///
fn set_sigint_handler(mut bus: Bus<()>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        let _ = bus.broadcast(());
    })
    .expect("Error setting Ctrl-C handler");
}

///
fn meter() {
    todo!();
}

///
fn st_send(
    args: &Args,
    net_opt: &mut nethuns_socket_options,
    out_sockets: &mut Vec<*mut nethuns_socket_t>,
    payload: &[u_char],
    mut bus_rx: BusReader<()>,
) -> Result<(), SendError> {
    // one packet index per socket (pos of next slot/packet to send in tx ring)
    let pktid: Vec<u64> = vec![0; args.num_sockets as usize];
    
    // Error buffer
    let mut errbuf: [c_char; NETHUNS_ERRBUF_SIZE as usize] = [0; NETHUNS_ERRBUF_SIZE as usize];
    
    for i in 0..args.num_sockets {
        fill_tx_ring(
            i,
            &mut out_sockets[i as usize],
            args,
            net_opt,
            &payload,
            &mut errbuf,
        )?;
    }
    
    loop {
        match bus_rx.try_recv() {
            Ok(()) => break,
            Err(_) => {
                for i in 0..args.num_sockets {
                    if args.zerocopy {
                        transmit_zc(i);
                    } else {
                        transmit_c(i);
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Setup and fill transmission ring
fn fill_tx_ring(
    th_idx: c_int,
    out_socket: &mut *mut nethuns_socket_netmap,
    args: &Args,
    net_opt: &mut nethuns_socket_options,
    payload: &[c_uchar],
    errbuf: &mut [c_char],
) -> Result<(), SendError> {
    assert!(payload.as_ptr().is_null() == false);
    
    // Open socket
    *out_socket = unsafe { nethuns_open_netmap(net_opt, errbuf.as_mut_ptr()) };
    if (*out_socket).is_null() {
        return Err(SendError::Exception(char_array_to_string(errbuf)));
    }
    
    assert!(out_socket.is_null() == false);
    
    let queue_len = if args.num_sockets > 1 {
        th_idx
    } else {
        NETHUNS_ANY_QUEUE
    };
    let result = unsafe { nethuns_bind_netmap(*out_socket, args.interface.as_ptr(), queue_len) };
    if result < 0 {
        return Err(SendError::NethunsException(*out_socket));
    }
    
    // fill the slots in the tx ring (optimized send only)
    if args.zerocopy {
        let size = unsafe { nethuns_txring_get_size(*out_socket) };
        
        for j in 0..size {
            // tell me where to copy the j-th packet to be transmitted
            let pkt = unsafe { nethuns_get_buf_addr(*out_socket, j as u64) };
            
            assert!(pkt.is_null() == false);
            
            // copy the packet
            unsafe {
                memcpy(
                    pkt as *mut c_void,
                    payload.as_ptr() as *const c_void,
                    payload.len() as u64,
                );
            }
        }
    }
    
    Ok(())
}

/// transmit packets in the tx ring (use optimized send, zero copy)
fn transmit_zc(out_socket: &mut *mut nethuns_socket_netmap, pktid: &mut u64) {
    // // prepare batch
    // for (int n = 0; n < batch_size; n++) {
    //     if (nethuns_send_slot(out[th_idx], pktid[th_idx], pkt_size) <= 0)
    //         break;
    //     pktid[th_idx]++;
    //     totals.at(th_idx)++;
    // }
    // nethuns_flush(out[th_idx]);             // send batch
    
    for n in 0..args.batch_size {
        let x = unsafe { nethuns_send_slot(out_sockets, pktid[i as usize], 1) };
    }
}

///
fn transmit_c(i: c_int) {
    todo!();
}

///
fn mt_send() {
    todo!();
}
