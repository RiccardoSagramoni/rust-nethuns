#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::error::Error;
use std::ffi::{CStr, CString};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{env, ptr, thread};

use bus::{Bus, BusReader};
use libc::{c_char, c_int, c_uchar, c_ulong};
use rust_nethuns::*;


#[derive(Debug)]
struct Args {
    interface: CString,
    num_sockets: c_int,
    multithreading: bool,
    sockstats: Option<c_int>,
    debug: bool,
}


#[derive(Debug)]
enum MeterError {
    NethunsException(*mut nethuns_socket_t),
    Exception(String),
}


#[derive(Debug)]
struct sync_nethuns_socket_t_pointer(*mut nethuns_socket_t);

unsafe impl Send for sync_nethuns_socket_t_pointer {}
unsafe impl Sync for sync_nethuns_socket_t_pointer {}


const HELP_BRIEF: &str = "\
Usage:  meter [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -n <nsock> ]      set number of sockets
            [ -m ]              enable multithreading
            [ -s <sockid> ]     enable per-socket stats
            [ -d ]              enable extra debug printing
";

const HELP_LONG: &str = "\
Usage:  meter [ options ]

-h, --help                      Show program usage and exit

Required options:

-i, --interface     <ifname>    Name of the network interface that meter operates on.

Other options:

-n, --sockets       <nsock>     Number of sockets to use. By default, only one socket is used.

-m, --multithreading            Enable multithreading. By default, only one thread is used.
                                If multithreading is enabled, and there is more than 
                                one socket in use, each socket is handled by a separated thread.

-s, --sockstats     <sockid>    Enable printing of complete statistics for the <sockid> socket 
                                in range [0, nsock).
                                By default, aggregated statistics for all the sockets in use 
                                are printed out.

-d, --debug                     Enable printing of extra info out to stdout for debug purposes
                                (e.g., IP address fields of received packets).
";


fn main() {
    let (args, mut net_opt) = configure_example();
    
    // Vector for storing socket ids
    let mut out_sockets: Vec<Arc<sync_nethuns_socket_t_pointer>> = Vec::new();
    
    // Setup sockets and rings
    for i in 0..args.num_sockets {
        let mut socket: *mut nethuns_socket_t = ptr::null_mut();
        match setup_rx_ring(&args, &mut net_opt, i as c_int, &mut socket) {
            Ok(_) => {
                out_sockets
                    .push(Arc::new(sync_nethuns_socket_t_pointer(socket)));
            }
            Err(e) => {
                panic!("Error in setup_rx_ring: {:?}", e);
            }
        }
    }
    
    // Stats counter
    let totals: Arc<Mutex<Vec<u64>>> =
        Arc::new(Mutex::new(vec![0; args.num_sockets as usize]));
    
    // Define bus for SPMC communication between threads
    let mut bus: Bus<()> = Bus::new(5);
    
    // Create thread for computing statistics
    let stats_th = if let Some(sockstats) = args.sockstats {
        let socket = out_sockets[sockstats as usize].clone();
        let totals = totals.clone();
        let rx = bus.add_rx();
        thread::spawn(move || sock_meter(socket, totals, rx))
    } else {
        let totals = totals.clone();
        let rx = bus.add_rx();
        thread::spawn(move || global_meter(totals, rx))
    };
    
    if !args.multithreading {
        // case single thread (main) with generic number of sockets
        let rx = bus.add_rx();
        set_sigint_handler(bus);
        st_execution(&args, out_sockets, rx, totals);
    } else {
        // // case multithreading enabled (num_threads == num_sockets)
        // let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
        // let args = Arc::new(args);
        
        // for th_idx in 0..args.num_sockets {
        //     let args = args.clone();
        //     let rx = bus.add_rx();
        //     let totals = totals.clone();
        //     threads.push(thread::spawn(move || {
        //         mt_execution(args, &mut net_opt, th_idx, rx, totals)
        //     }));
        // }
        
        // set_sigint_handler(bus);
        
        // for t in threads {
        //     if let Err(e) = t.join() {
        //         eprintln!("Error joining thread: {:?}", e);
        //     }
        // }
        todo!()
    }
    
    if let Err(e) = stats_th.join() {
        eprintln!("Error joining stats thread: {:?}", e);
    }
}


///
fn configure_example() -> (Args, nethuns_socket_options) {
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
    
    // Nethuns options
    let net_opt: nethuns_socket_options = nethuns_socket_options {
        numblocks: 1,
        numpackets: 4096,
        packetsize: 2048,
        timeout_ms: 0,
        dir: nethuns_capture_dir_nethuns_in_out,
        capture: nethuns_capture_mode_nethuns_cap_zero_copy,
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
    dbg!(net_opt);
    
    (args, net_opt)
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
        interface: CString::new::<String>(
            pargs.value_from_str(["-i", "--interface"])?,
        )?,
        num_sockets: pargs.value_from_str(["-n", "--sockets"]).unwrap_or(1),
        multithreading: pargs.contains(["-m", "--multithreading"]),
        sockstats: pargs.value_from_str(["-s", "--sockstats"]).ok(),
        debug: pargs.contains(["-d", "--debug"]),
    };
    
    // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }
    
    Ok(args)
}

/// Converts a slice of c_char to a String.
fn char_array_to_string(arr: &[c_char]) -> String {
    unsafe {
        return CStr::from_ptr(arr.as_ptr())
            .to_owned()
            .into_string()
            .expect("into_string() call failed");
    }
}


/// Set an handler for the SIGINT signal (Ctrl-C),
/// which will notify the other threads
/// to gracefully stop their execution.
///
/// # Arguments
/// - `bus`: Bus for SPMC (single-producer/multiple-consumers) communication
///   between threads.
fn set_sigint_handler(mut bus: Bus<()>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        bus.broadcast(());
    })
    .expect("Error setting Ctrl-C handler");
}


///
fn global_meter(totals: Arc<Mutex<Vec<u64>>>, mut rx: BusReader<()>) {
    let mut now = SystemTime::now();
    let mut old_total: u64 = 0;
    
    loop {
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => (),
        }
        
        // Sleep for 1 second
        let next_sys_time = now
            .checked_add(Duration::from_secs(1))
            .expect("SystemTime::checked_add() failed");
        if let Ok(delay) = next_sys_time.duration_since(now) {
            thread::sleep(delay);
        }
        now = next_sys_time;
        
        // Print number of sent packets
        let new_total: u64 = totals.lock().expect("lock failed").iter().sum();
        println!("pkt/sec: {}", new_total - old_total);
        old_total = new_total;
    }
}


///
fn sock_meter(
    socket: Arc<sync_nethuns_socket_t_pointer>,
    totals: Arc<Mutex<Vec<u64>>>,
    mut rx: BusReader<()>,
) {
    let mut now = SystemTime::now();
    let mut old_total: u64 = 0;
    
    loop {
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => (),
        }
        
        // Sleep for 1 second
        let next_sys_time = now
            .checked_add(Duration::from_secs(1))
            .expect("SystemTime::checked_add() failed");
        if let Ok(delay) = next_sys_time.duration_since(now) {
            thread::sleep(delay);
        }
        now = next_sys_time;
        
        // Print number of sent packets + stats about the requestes socket
        let new_total: u64 = totals.lock().expect("lock failed").iter().sum();
        print!("pkt/sec: {} ", new_total - old_total);
        old_total = new_total;
        
        let mut stats = nethuns_stat::default();
        unsafe {
            nethuns_stats_netmap(socket.0, &mut stats);
            println!(
                "{{ rx: {}, tx: {}, drop: {}, ifdrop: {}, rx_inv: {}, tx_inv: {}, freeze: {} }}", 
                stats.rx_packets, stats.tx_packets,
                stats.rx_dropped, stats.rx_if_dropped,
                stats.rx_invalid, stats.tx_invalid,
                stats.freeze
            );
        }
    }
}


///
fn setup_rx_ring(
    args: &Args,
    net_opt: &mut nethuns_socket_options,
    socket_idx: c_int,
    socket: &mut *mut nethuns_socket_t,
) -> Result<(), MeterError> {
    // Error buffer
    let mut errbuf: [c_char; NETHUNS_ERRBUF_SIZE as usize] =
        [0; NETHUNS_ERRBUF_SIZE as usize];
    
    (*socket) = unsafe { nethuns_open_netmap(net_opt, errbuf.as_mut_ptr()) };
    if (*socket).is_null() {
        return Err(MeterError::Exception(char_array_to_string(&errbuf)));
    }
    
    let queue_len = if args.num_sockets > 1 {
        socket_idx
    } else {
        NETHUNS_ANY_QUEUE
    };
    
    if unsafe {
        nethuns_bind_netmap(*socket, args.interface.as_ptr(), queue_len)
    } < 0
    {
        return Err(MeterError::NethunsException(*socket));
    }
    
    if args.debug {
        println!(
            "Thread: {}, bind on {:?}:{}",
            socket_idx, args.interface, queue_len
        );
    }
    
    Ok(())
}


///
fn st_execution(
    args: &Args,
    sockets: Vec<Arc<sync_nethuns_socket_t_pointer>>,
    rx: BusReader<()>,
    totals: Arc<Mutex<Vec<u64>>>,
) {
    if let Err(e) = st_rcv(args, sockets, rx, totals) {
        match e {
            MeterError::NethunsException(s) => {
                if !s.is_null() {
                    unsafe {
                        nethuns_close_netmap(s);
                    }
                }
                eprintln!("Nethuns socket failed: {:?}", s);
                std::process::exit(1);
            }
            MeterError::Exception(e) => {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}


///
fn st_rcv(
    args: &Args,
    sockets: Vec<Arc<sync_nethuns_socket_t_pointer>>,
    mut rx: BusReader<()>,
    mut totals: Arc<Mutex<Vec<u64>>>,
) -> Result<(), MeterError> {
    loop {
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => (),
        }
        
        let mut count_to_dump: c_ulong = 0;
        for i in 0..args.num_sockets {
            recv_pkt(
                args,
                i,
                &mut count_to_dump,
                &sockets[i as usize],
                &mut totals,
            )?;
        }
        
        if args.debug {
            println!("Thread: MAIN, count to dump: {count_to_dump}");
        }
    }
    
    // Close sockets
    for s in sockets {
        unsafe {
            nethuns_close_netmap(s.0);
        }
    }
    
    Ok(())
}


/// receive and process a packet
fn recv_pkt(
    args: &Args,
    th_idx: c_int,
    count_to_dump: &mut c_ulong,
    socket: &Arc<sync_nethuns_socket_t_pointer>,
    totals: &mut Arc<Mutex<Vec<u64>>>,
) -> Result<(), MeterError> {
    let mut pkthdr: *const nethuns_pkthdr_t = ptr::null_mut();
    let mut frame: *const c_uchar = ptr::null_mut();
    let pkt_id: u64 =
        unsafe { nethuns_recv_netmap(socket.0, &mut pkthdr, &mut frame) };
    
    if pkt_id == u64::MAX {
        return Err(MeterError::NethunsException(socket.0));
    } else if pkt_id == 0 {
        return Ok(());
    }
    
    // Process valid packet here
    if args.debug {
        println!(
            "Thread: {}, total: {}, pkt: {}",
            th_idx,
            totals.lock().expect("lock failed")[th_idx as usize],
            pkt_id
        );
        println!("Packet IP addr: {}", print_addrs(&frame));
    }
    
    totals.lock().expect("lock failed")[th_idx as usize] += 1;
    *count_to_dump += 1;
    if *count_to_dump == 10000000 {
        // do something periodically
        *count_to_dump = 0;
        unsafe { nethuns_dump_rings_netmap(socket.0) }
    }
    
    nethuns_rx_release(socket.0, pkt_id);
    
    Ok(())
}


///
fn print_addrs(_frame: &*const c_uchar) -> String {
    // TODO
    todo!()
}
