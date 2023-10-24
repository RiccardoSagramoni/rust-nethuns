use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use bus::{Bus, BusReader};
use etherparse::{IpHeader, PacketHeaders};
use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};


const HELP_BRIEF: &str = "\
Usage:  meter [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -n <nsock> ]      set number of sockets
            [ -m ]              enable multithreading
            [ -s <sockid> ]     enable per socket stats
            [ -d ]              enable extra debug printing
";

const HELP_LONG: &str = "\
Usage:  send [ options ]

-h, --help                      Show program usage and exit

Required options:

-i, --interface     <ifname>    Name of the network interface that send operates on.

Other options:

-n, --sockets       <nsock>     Number of sockets to use. By default, only one socket is used.

-m, --multithreading            Enable multithreading. By default, only one thread is used.
                                If multithreading is enabled, and there is more than one socket in use,
                                each socket is handled by a separated thread.

-s, --sockstats     <sockid>    Enable printing of complete statistics for the <sockid> socket in range [0, nsock).

-d, --debug                     Enable printing of extra info out to stdout for debug purposes
                                (e.g., IP address fields of received packets).
";


#[derive(Debug, Default)]
struct Configuration {
    interface: String,
    num_sockets: u32,
    multithreading: bool,
    sockstats: Option<u32>,
    debug: bool,
}


fn main() {
    let conf = match get_configuration() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error in parsing command line options: {e}.");
            eprint!("{}", HELP_BRIEF);
            return;
        }
    };
    
    let nethuns_opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 4096,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::ZeroCopy,
        mode: NethunsSocketMode::RxTx,
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    // Open sockets
    let mut sockets: Vec<Mutex<NethunsSocket>> =
        Vec::with_capacity(conf.num_sockets as _);
    for i in 0..sockets.capacity() {
        sockets.push(Mutex::new(setup_rx_ring(
            &conf,
            nethuns_opt.clone(),
            i as _,
        )));
    }
    let sockets = Arc::new(sockets);
    
    
    // Stats counter
    let mut totals: Vec<AtomicU64> = Vec::with_capacity(conf.num_sockets as _);
    for _ in 0..totals.capacity() {
        totals.push(AtomicU64::new(0));
    }
    let totals = Arc::new(totals);
    
    // Define bus for SPMC communication between threads
    let mut sigint_bus: Bus<()> = Bus::new(5);
    
    // Create a thread for computing statistics
    let meter_thread = {
        let totals = totals.clone();
        let sigint_rx = sigint_bus.add_rx();
        match conf.sockstats {
            Some(sockid) => {
                let sockets = sockets.clone();
                thread::spawn(move || {
                    sock_meter(
                        sockid,
                        &sockets[sockid as usize],
                        totals,
                        sigint_rx,
                    )
                })
            }
            None => thread::spawn(move || global_meter(totals, sigint_rx)),
        }
    };
    
    
    if !conf.multithreading {
        // case single thread (main) with generic number of sockets
        let sigint_rx = sigint_bus.add_rx();
        set_sigint_handler(sigint_bus);
        st_execution(&conf, sockets, totals, sigint_rx)
            .expect("MAIN thread execution failed");
    } else {
        // case multithreading enabled (num_threads == num_sockets)
        let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
        let conf = Arc::new(conf);
        
        for th_idx in 0..conf.num_sockets {
            let conf = conf.clone();
            let rx = sigint_bus.add_rx();
            let sockets = sockets.clone();
            let totals = totals.clone();
            threads.push(thread::spawn(move || {
                mt_execution(
                    &conf,
                    th_idx,
                    &sockets[th_idx as usize],
                    &totals[th_idx as usize],
                    rx,
                )
                .unwrap_or_else(|_| panic!("Thread {th_idx} execution failed"));
            }));
        }
        
        set_sigint_handler(sigint_bus);
        
        for t in threads {
            if let Err(e) = t.join() {
                eprintln!("Error joining thread: {:?}", e);
            }
        }
    }
    
    if let Err(e) = meter_thread.join() {
        eprintln!("Error joining stats thread: {:?}", e);
    }
}


// Parses the command-line arguments and build an instance of the `Args`
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
fn get_configuration() -> Result<Configuration, anyhow::Error> {
    let mut args = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if args.contains(["-h", "--help"]) {
        print!("{}", HELP_LONG);
        std::process::exit(0);
    }
    
    let conf = Configuration {
        interface: args.value_from_str(["-i", "--interface"])?,
        num_sockets: args.value_from_str(["-n", "--sockets"]).unwrap_or(1),
        multithreading: args.contains(["-m", "--multithreading"]),
        sockstats: args.value_from_str(["-s", "--sockstats"]).ok(),
        debug: args.contains(["-d", "--debug"]),
    };
    
    // It's up to the caller what to do with the remaining arguments.
    let remaining = args.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }
    
    println!(
        "\
Test {} started with parameters
* interface: {}
* sockets: {}
* multithreading: {}
* sockstats: {}
* debug: {}
",
        std::env::args().next().unwrap(),
        conf.interface,
        conf.num_sockets,
        if conf.multithreading { "ON" } else { "OFF" },
        if let Some(sockid) = conf.sockstats {
            format!("ON for socket {sockid}")
        } else {
            "OFF, aggregated stats only".to_owned()
        },
        if conf.debug { "ON" } else { "OFF" },
    );
    
    Ok(conf)
}


fn setup_rx_ring(
    conf: &Configuration,
    opt: NethunsSocketOptions,
    sockid: u32,
) -> NethunsSocket {
    let socket = BindableNethunsSocket::open(opt)
        .expect("Failed to open nethuns socket")
        .bind(
            &conf.interface,
            if conf.num_sockets > 1 {
                NethunsQueue::Some(sockid)
            } else {
                NethunsQueue::Any
            },
        )
        .expect("Failed to bind nethuns socket");
    
    if conf.debug {
        println!(
            "Thread: {}, bind on {}:{}",
            sockid,
            conf.interface,
            if conf.num_sockets > 1 {
                sockid as i64
            } else {
                -1
            }
        );
    }
    
    socket
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


fn global_meter(totals: Arc<Vec<AtomicU64>>, mut sigint_rx: BusReader<()>) {
    let mut now = SystemTime::now();
    
    loop {
        match sigint_rx.try_recv() {
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
        let total: u64 =
            totals.iter().map(|t| t.swap(0, Ordering::AcqRel)).sum();
        println!("pkt/sec: {}", total.to_formatted_string(&Locale::en));
    }
}


/// Print aggregated stats and per-socket detailed stats
fn sock_meter(
    sockid: u32,
    socket: &Mutex<NethunsSocket>,
    totals: Arc<Vec<AtomicU64>>,
    mut rx: BusReader<()>,
) {
    let mut now = SystemTime::now();
    
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
        let total_sock = totals[sockid as usize].load(Ordering::Acquire);
        let total: u64 =
            totals.iter().map(|t| t.swap(0, Ordering::AcqRel)).sum();
        print!("pkt/sec: {} ", total.to_formatted_string(&Locale::en));
        
        let stats = socket
            .lock()
            .expect("Mutex::lock failed for `socket`")
            .stats()
            .expect("NethunsSocket::stats failed");
        println!(
            "{{ pkt/sec: {}, rx: {}, tx: {}, drop: {}, ifdrop: {}, rx_inv: {}, tx_inv: {}, freeze: {} }}",
            total_sock,
            stats.rx_packets(), stats.tx_packets(),
            stats.rx_dropped(), stats.rx_if_dropped(),
            stats.rx_invalid(), stats.tx_invalid(),
            stats.freeze()
        );
    }
}


fn st_execution(
    conf: &Configuration,
    sockets: Arc<Vec<Mutex<NethunsSocket>>>,
    totals: Arc<Vec<AtomicU64>>,
    mut sigint_rx: BusReader<()>,
) -> anyhow::Result<()> {
    let mut count_to_dump: u64 = 0;
    
    loop {
        // Check if Ctrl-C was pressed
        match sigint_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        for (id, (sock, tot)) in sockets.iter().zip(totals.iter()).enumerate() {
            let sock = sock
                .lock()
                .map_err(|e| anyhow::anyhow!("Error locking mutex: {e}"))?;
            
            match recv_pkt(conf, id, &sock, tot, &mut count_to_dump) {
                Ok(_) => (),
                Err(e) => match e.downcast_ref::<NethunsRecvError>() {
                    Some(NethunsRecvError::InUse)
                    | Some(NethunsRecvError::NoPacketsAvailable)
                    | Some(NethunsRecvError::PacketFiltered) => (),
                    _ => return Err(e),
                },
            }
        }
    }
    
    if conf.debug {
        println!("Thread: MAIN, count to dump: {}", count_to_dump);
    }
    
    Ok(())
}


fn mt_execution(
    conf: &Configuration,
    sockid: u32,
    socket: &Mutex<NethunsSocket>,
    total: &AtomicU64,
    mut sigint_rx: BusReader<()>,
) -> anyhow::Result<()> {
    let mut count_to_dump: u64 = 0;
    
    loop {
        // Check if Ctrl-C was pressed
        match sigint_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        match recv_pkt(
            conf,
            sockid as _,
            &socket.lock().expect("Mutex::lock failed for `socket`"),
            total,
            &mut count_to_dump,
        ) {
            Ok(_) => (),
            Err(e) => match e.downcast_ref::<NethunsRecvError>() {
                Some(NethunsRecvError::InUse)
                | Some(NethunsRecvError::NoPacketsAvailable)
                | Some(NethunsRecvError::PacketFiltered) => (),
                _ => return Err(e),
            },
        }
    }
    
    if conf.debug {
        println!("Thread: {sockid}, count to dump: {count_to_dump}");
    }
    
    Ok(())
}


fn recv_pkt(
    conf: &Configuration,
    sockid: usize,
    socket: &NethunsSocket,
    total: &AtomicU64,
    count_to_dump: &mut u64,
) -> anyhow::Result<()> {
    let pkt = socket.recv()?;
    
    let old_total = total.fetch_add(1, Ordering::AcqRel);
    
    if conf.debug {
        println!(
            "Thread: {}, total: {}, pkt: {}",
            sockid,
            old_total,
            pkt.id()
        );
        println!("Packet IP addr: {}", print_addrs(pkt.buffer())?);
    }
    
    *count_to_dump += 1;
    if *count_to_dump == 10_000_000 {
        // do something periodically
        *count_to_dump = 0;
        socket.dump_rings();
    }
    
    Ok(())
}


fn print_addrs(frame: &[u8]) -> anyhow::Result<String> {
    // Parse the ethernet header
    let packet_header = PacketHeaders::from_ethernet_slice(frame)?;
    
    // Get reference to IP header
    let ip_header = &packet_header
        .ip
        .ok_or(anyhow::anyhow!("Error: IP header not found"))?;
    
    match ip_header {
        IpHeader::Version4(hdr, _) => Ok(format!(
            "IP: {} > {}",
            Ipv4Addr::from(hdr.source),
            Ipv4Addr::from(hdr.destination)
        )),
        IpHeader::Version6(hdr, _) => Ok(format!(
            "IP: {} > {}",
            Ipv6Addr::from(hdr.source),
            Ipv6Addr::from(hdr.destination)
        )),
    }
}
