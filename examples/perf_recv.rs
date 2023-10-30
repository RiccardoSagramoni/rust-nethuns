use std::sync::mpsc::TryRecvError;
use std::time::{Duration, SystemTime};
use std::{env, mem};

use bus::Bus;
use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};


#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;


fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    
    let dev = env::args().nth(1).expect("Usage: ./bench_recv <dev>");
    
    let nethuns_opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 4096,
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
    
    // Open sockets
    let socket = BindableNethunsSocket::open(nethuns_opt)
        .unwrap()
        .bind(&dev, NethunsQueue::Any)
        .unwrap();
    
    // Define bus for SPMC communication between threads
    let mut sigint_bus: Bus<()> = Bus::new(5);
    
    // Set handler for Ctrl-C
    let mut sigint_rx = sigint_bus.add_rx();
    set_sigint_handler(sigint_bus);
    
    // Start receiving
    let mut total: u64 = 0;
    let mut time_for_logging = SystemTime::now()
        .checked_add(Duration::from_secs(1))
        .unwrap();
    
    loop {
        // Check if Ctrl-C was pressed
        match sigint_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        if time_for_logging < SystemTime::now() {
            let total = mem::replace(&mut total, 0);
            println!("pkt/sec: {}", total.to_formatted_string(&Locale::en));
            time_for_logging = SystemTime::now()
                .checked_add(Duration::from_secs(1))
                .unwrap();
        }
        
        match socket.recv() {
            Ok(_) => {
                total += 1;
            }
            Err(NethunsRecvError::InUse)
            | Err(NethunsRecvError::NoPacketsAvailable)
            | Err(NethunsRecvError::PacketFiltered) => (),
            Err(e) => panic!("Error: {e}"),
        }
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
