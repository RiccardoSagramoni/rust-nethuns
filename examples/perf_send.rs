use std::io::Write;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, SystemTime};
use std::{env, mem};

use bus::Bus;
use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};


#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;


#[derive(Debug)]
struct Args {
    interface: String,
    batch_size: u32,
    zerocopy: bool,
}


const HELP_BRIEF: &str = "\
Usage:  send [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -b <batch_sz> ]   set batch size
            [ -z ]              enable send zero-copy
";


fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    
    let (args, payload, opt) = configure_example();
    
    // Define bus for SPMC communication between threads
    let mut bus: Bus<()> = Bus::new(5);
    
    // case single thread (main) with generic number of sockets
    let mut bus_rx = bus.add_rx();
    set_sigint_handler(bus);
    
    
    // Setup and fill transmission rings for each socket
    let socket = prepare_tx_socket(&args, opt, &payload).unwrap();
    
    // One packet index per socket (pos of next slot/packet to send in tx ring)
    let mut pktid: usize = 0;
    
    let mut total: u64 = 0;
    let mut time_for_logging = SystemTime::now()
        .checked_add(Duration::from_secs(1))
        .unwrap();
    
    loop {
        // Check if Ctrl-C was pressed
        match bus_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        // Check if enough time has passed for printing stats
        if time_for_logging < SystemTime::now() {
            let total = mem::replace(&mut total, 0);
            println!("pkt/sec: {}", total.to_formatted_string(&Locale::en));
            time_for_logging = SystemTime::now()
                .checked_add(Duration::from_secs(1))
                .unwrap();
        }
        
        // Transmit packets from each socket
        if args.zerocopy {
            transmit_zc(&args, &socket, &mut pktid, payload.len(), &mut total)
                .unwrap();
        } else {
            transmit_c(&args, &socket, &payload, &mut total).unwrap();
        }
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
        print!("{}", HELP_BRIEF);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: pargs.value_from_str(["-i", "--interface"])?,
        batch_size: pargs.value_from_str(["-b", "--batch_size"]).unwrap_or(1),
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


/// Setup and fill transmission ring.
fn prepare_tx_socket(
    args: &Args,
    opt: NethunsSocketOptions,
    payload: &[u8],
) -> Result<NethunsSocket, anyhow::Error> {
    // Open socket
    let mut socket = BindableNethunsSocket::open(opt)?
        .bind(&args.interface, NethunsQueue::Any)
        .map_err(|(e, _)| e)?;
    
    // fill the slots in the tx ring (optimized send only)
    if args.zerocopy {
        let size = socket.txring_get_size().expect("socket not in tx mode");
        
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
fn transmit_zc(
    args: &Args,
    socket: &NethunsSocket,
    pktid: &mut usize,
    pkt_size: usize,
    total: &mut u64,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if socket.send_slot(*pktid, pkt_size).is_err() {
            break;
        }
        *pktid += 1;
        *total += 1;
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
    socket: &NethunsSocket,
    payload: &[u8],
    totals: &mut u64,
) -> Result<(), anyhow::Error> {
    // Prepare batch
    for _ in 0..args.batch_size {
        if socket.send(payload).is_err() {
            break;
        }
        *totals += 1;
    }
    // Send batch
    socket.flush()?;
    Ok(())
}
