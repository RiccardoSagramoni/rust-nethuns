use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{env, thread};

use bus::{Bus, BusReader};
use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};


fn main() {
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
    
    // Stats counter
    let total = Arc::new(AtomicU64::new(0));
    
    // Define bus for SPMC communication between threads
    let mut sigint_bus: Bus<()> = Bus::new(5);
    
    // Create a thread for computing statistics
    let meter_thread = {
        let total = total.clone();
        let sigint_rx = sigint_bus.add_rx();
        thread::spawn(move || meter(total, sigint_rx))
    };
    
    // Set handler for Ctrl-C
    let mut sigint_rx = sigint_bus.add_rx();
    set_sigint_handler(sigint_bus);
    
    // Start receiving
    loop {
        // Check if Ctrl-C was pressed
        match sigint_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        match socket.recv() {
            Ok(_) => {
                total.fetch_add(1, Ordering::AcqRel);
            }
            Err(NethunsRecvError::InUse)
            | Err(NethunsRecvError::NoPacketsAvailable)
            | Err(NethunsRecvError::PacketFiltered) => (),
            Err(e) => panic!("Error: {e}"),
        }
    }
    
    meter_thread.join().unwrap();
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


fn meter(total: Arc<AtomicU64>, mut sigint_rx: BusReader<()>) {
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
        let total = total.swap(0, Ordering::AcqRel);
        println!("pkt/sec: {}", total.to_formatted_string(&Locale::en));
    }
}
