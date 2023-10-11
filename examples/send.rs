use std::io::Write;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{env, thread};

use bus::{Bus, BusReader};
use nethuns::sockets::ring::txring_get_size;
use nethuns::sockets::{nethuns_socket_open, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};


const HELP_BRIEF: &str = "\
Usage:  send [ options ]
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
Usage:  send [ options ]

-h, --help                      Show program usage and exit

Required options:

-i, --interface     <ifname>    Name of the network interface that send operates on.

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
    interface: String,
    batch_size: u32,
    num_sockets: u32,
    multithreading: bool,
    zerocopy: bool,
}


fn main() {
    let (args, payload, opt) = configure_example();
    
    // Stats counter
    let totals: Arc<Mutex<Vec<u64>>> =
        Arc::new(Mutex::new(vec![0; args.num_sockets as _]));
    
    // Define bus for SPMC communication between threads
    let mut bus: Bus<()> = Bus::new(5);
    
    // Create a thread for computing statistics
    let stats_th = {
        let totals = totals.clone();
        let rx = bus.add_rx();
        thread::spawn(move || {
            meter(totals, rx);
        })
    };
    
    if !args.multithreading {
        // case single thread (main) with generic number of sockets
        let rx = bus.add_rx();
        set_sigint_handler(bus);
        st_send(&args, opt, &payload, rx, totals)
            .expect("MAIN thread execution failed: {e}");
    } else {
        // case multithreading enabled (num_threads == num_sockets)
        let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
        let args = Arc::new(args);
        
        for th_idx in 0..args.num_sockets {
            let args = args.clone();
            let opt = opt.clone();
            let rx = bus.add_rx();
            let totals = totals.clone();
            threads.push(thread::spawn(move || {
                mt_send(&args, opt, th_idx, &payload, rx, totals).expect(
                    format!("Thread {th_idx} execution failed").as_str(),
                );
            }));
        }
        
        set_sigint_handler(bus);
        
        for t in threads {
            if let Err(e) = t.join() {
                eprintln!("Error joining thread: {:?}", e);
            }
        }
    }
    
    if let Err(e) = stats_th.join() {
        eprintln!("Error joining stats thread: {:?}", e);
    }
}


/// Configures the example for sending packets, by parsing the command line
/// arguments and filling the default payload and nethuns options.
///
/// # Returns
///
/// A tuple containing the parsed arguments, the payload and the nethuns
/// options.
fn configure_example() -> (Args, [u8; 34], NethunsSocketOptions) {
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
    let payload: [u8; 34] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
        0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
        0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
        0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
        0x07, 0x08,
    ];
    
    // Nethuns options
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 2048,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::ZeroCopy,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    (args, payload, opt)
}


/// Parses the command-line arguments and build an instance of the `Args`
/// struct.
///
/// It uses the `pico_args` crate to handle argument parsing.
///
/// # Returns
///
/// - `Ok(Args)`: If the command-line arguments are successfully parsed, a
///   Result with an Args instance containing the parsed options is returned.
/// - `Err(anyhow::Error)`: If an error occurs during argument parsing or
///   any related operations, a Result with a boxed error is returned.
fn parse_args() -> Result<Args, anyhow::Error> {
    let mut pargs = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP_LONG);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: pargs.value_from_str(["-i", "--interface"])?,
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


/// Meter the number of sent packets.
///
/// # Arguments
///
/// - `totals`: Vector for storing the number of sent packets from each socket.
///   It's shared between threads.
/// - `rx`: BusReader for SPMC (single-producer/multiple-consumers)
///   communication between threads.
fn meter(totals: Arc<Mutex<Vec<u64>>>, mut rx: BusReader<()>) {
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


/// Execute packets transmission for single-threaded example.
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `opt`: Nethuns socket options.
/// - `payload`: Payload for packets.
/// - `rx`: BusReader for SPMC (single-producer/multiple-consumers)
///   communication between threads.
/// - `totals`: Vector for storing the number of packets sent from each socket.
///
/// # Returns
/// - `Ok(())`: If transmission is successful.
/// - `Err(anyhow::Error)`: If an error occurs during transmission.
fn st_send(
    args: &Args,
    opt: NethunsSocketOptions,
    payload: &[u8],
    mut bus_rx: BusReader<()>,
    totals: Arc<Mutex<Vec<u64>>>,
) -> Result<(), anyhow::Error> {
    // Vector for storing socket ids
    let mut out_sockets: Vec<Box<dyn NethunsSocket>> =
        Vec::with_capacity(args.num_sockets as _);
    // One packet index per socket (pos of next slot/packet to send in tx ring)
    let mut pktid: Vec<usize> = vec![0; args.num_sockets as _];
    
    // Setup and fill transmission rings for each socket
    for i in 0..args.num_sockets {
        out_sockets.push(fill_tx_ring(args, opt.clone(), i, payload)?);
    }
    
    loop {
        // Check if Ctrl-C was pressed
        match bus_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        // Transmit packets from each socket
        for (i, socket) in out_sockets.iter_mut().enumerate() {
            if args.zerocopy {
                transmit_zc(
                    args,
                    socket,
                    pktid.get_mut(i).expect("pktid.get_mut() failed"),
                    payload.len(),
                    &totals,
                    i,
                )?;
            } else {
                transmit_c(args, socket, payload, &totals, i)?;
            }
        }
    }
    
    Ok(())
}


/// Execute packets transmission for multi-threaded example.
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `opt`: Nethuns socket options.
/// - `th_idx`: Thread index.
/// - `payload`: Payload for packets.
/// - `rx`: BusReader for SPMC (single-producer/multiple-consumers)
///   communication between threads.
/// - `totals`: Vector for storing the number of packets sent from each socket.
///
/// # Returns
/// - `Ok(())`: If transmission is successful.
/// - `Err(anyhow::Error)`: If an error occurs during transmission.
fn mt_send(
    args: &Args,
    opt: NethunsSocketOptions,
    th_idx: u32,
    payload: &[u8],
    mut rx: BusReader<()>,
    totals: Arc<Mutex<Vec<u64>>>,
) -> Result<(), anyhow::Error> {
    // Setup and fill transmission ring
    let mut socket = fill_tx_ring(args, opt, th_idx, payload)?;
    
    // Packet id (only for zero-copy transmission)
    let mut pktid = 0_usize;
    
    loop {
        // Check if Ctrl-C was pressed
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => (),
        }
        
        // Transmit packets
        if args.zerocopy {
            transmit_zc(
                args,
                &mut socket,
                &mut pktid,
                payload.len(),
                &totals,
                th_idx as _,
            )?
        } else {
            transmit_c(args, &mut socket, payload, &totals, th_idx as _)?
        }
    }
    
    Ok(())
}


/// Setup and fill transmission ring.
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `opt`: Nethuns socket options.
/// - `socket_idx`: Socket index.
/// - `payload`: Payload for packets.
///
/// # Returns
/// - `Ok(())`: If transmission is successful.
/// - `Err(SendError)`: If an error occurs during transmission.
fn fill_tx_ring(
    args: &Args,
    opt: NethunsSocketOptions,
    socket_idx: u32,
    payload: &[u8],
) -> Result<Box<dyn NethunsSocket>, anyhow::Error> {
    // Open socket
    let socket = nethuns_socket_open(opt)?;
    
    // Bind socket
    let queue = if args.num_sockets > 1 {
        NethunsQueue::Some(socket_idx)
    } else {
        NethunsQueue::Any
    };
    let socket = socket.bind(&args.interface, queue).map_err(|(e, _)| e)?;
    
    // fill the slots in the tx ring (optimized send only)
    if args.zerocopy {
        let size = txring_get_size(&*socket).expect("socket not in tx mode");
        
        for j in 0..size {
            // tell me where to copy the j-th packet to be transmitted
            let mut pkt = socket
                .get_packet_buffer_ref(j as _)
                .expect("socket not in tx mode");
            
            // copy the packet
            pkt.write_all(payload)?;
        }
    }
    
    Ok(socket)
}


/// Transmit packets in the tx ring (use optimized send, zero copy).
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `socket`: Socket descriptor.
/// - `pktid`: Current packet id.
/// - `pkt_size`: Packet size.
/// - `totals`: Vector for storing the number of packets sent from each socket.
/// - `socket_idx`: Socket index.
fn transmit_zc(
    args: &Args,
    socket: &mut Box<dyn NethunsSocket>,
    pktid: &mut usize,
    pkt_size: usize,
    totals: &Arc<Mutex<Vec<u64>>>,
    socket_idx: usize,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if let Err(e) = socket.send_slot(*pktid, pkt_size) {
            dbg!(e);
            break;
        }
        (*pktid) += 1;
        if let Some(t) = totals.lock().unwrap().get_mut(socket_idx) {
            *t += 1;
        }
    }
    // Send batch
    socket.flush()?;
    Ok(())
}


/// Transmit packets in the tx ring (use classic send, copy)
///
/// # Arguments
/// - `args`: Parsed command-line arguments.
/// - `socket`: Socket descriptor.
/// - `payload`: Payload for packets.
/// - `totals`: Vector for storing the number of packets sent from each socket.
/// - `socket_idx`: Socket index.
fn transmit_c(
    args: &Args,
    socket: &mut Box<dyn NethunsSocket>,
    payload: &[u8],
    totals: &Arc<Mutex<Vec<u64>>>,
    socket_idx: usize,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if let Err(e) = socket.send(payload) {
            eprintln!("Error in transmission for socket {socket_idx}: {e}");
            break;
        }
        if let Some(t) = totals.lock().unwrap().get_mut(socket_idx) {
            *t += 1;
        }
    }
    // Send batch
    socket.flush()?;
    Ok(())
}
