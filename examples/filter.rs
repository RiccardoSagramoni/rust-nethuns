use std::ops::DerefMut;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{env, mem, thread};

use bus::{Bus, BusReader};
use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::{nethuns_socket_open, PkthdrTrait};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns::vlan::{
    nethuns_vlan_tci, nethuns_vlan_tci_, nethuns_vlan_tpid, nethuns_vlan_tpid_,
    nethuns_vlan_vid,
};
use rand::Rng;

fn main() {
    // Parse args
    let dev = env::args()
        .nth(1)
        .expect("Usage: ./recv_test <device_name>");
    
    // Create socket
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 5000,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    let socket = nethuns_socket_open(opt).unwrap();
    let mut socket = socket.bind(&dev, NethunsQueue::Any).unwrap();
    
    // Set filter
    socket.base_mut().filter = Some(Box::new(simple_filter));
    
    
    // Stats counter
    let total = Arc::new(Mutex::<u64>::new(0));
    // Define bus for SPMC communication between threads
    let mut bus: Bus<()> = Bus::new(5);
    
    // Create a thread for computing statistics
    let stats_th = {
        let total = total.clone();
        let rx = bus.add_rx();
        thread::spawn(move || {
            meter(total, rx);
        })
    };
    
    // Set handler for Ctrl-C
    let mut bus_rx = bus.add_rx();
    set_sigint_handler(bus);
    
    
    let mut total2: u64 = 0;
    loop {
        // Check if Ctrl-C was pressed
        match bus_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        match socket.recv() {
            Ok(_) => {
                let mut total = total.lock().expect("lock failed");
                *total += 1;
                
                total2 += 1;
                
                if total2 == 10_000_000 {
                    total2 = 0;
                    socket.dump_rings();
                }
            }
            Err(NethunsRecvError::NoPacketsAvailable)
            | Err(NethunsRecvError::PacketFiltered) => {
                continue;
            }
            Err(e) => {
                eprintln!("[ERROR]: {}", e);
            }
        }
    }
    
    if let Err(e) = stats_th.join() {
        eprintln!("Error joining stats thread: {:?}", e);
    }
}


/// Print statistics about received packets
fn meter(total: Arc<Mutex<u64>>, mut rx: BusReader<()>) {
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
        
        // Print number of sent packets
        let x = mem::replace(total.lock().unwrap().deref_mut(), 0);
        println!("pkt/sec: {}", x);
    }
}


/// Filter for nethuns socket
fn simple_filter(pkthdr: &dyn PkthdrTrait, packet: &[u8]) -> bool {
    // Filter packets with a very small probability
    if rand::thread_rng().gen_bool(0.00001) {
        eprintln!("Rejected packet: {}", dump_packet(pkthdr, packet));
        false
    } else {
        true
    }
}


fn dump_packet(pkthdr: &dyn PkthdrTrait, packet: &[u8]) -> String {
    let mut builder = string_builder::Builder::new(1024);
    builder.append(format!(
        "{}:{} snap:{} len:{} offload{{tci:{:X} tpid:{:X}}} packet{{tci:{:X} pid:{:X}}} => [tci:{:X} tpid:{:X} vid:{:X}] rxhash:0x{:X} | ",
        pkthdr.tstamp_sec(),
        pkthdr.tstamp_nsec(),
        pkthdr.snaplen(),
        pkthdr.len(),
        pkthdr.offvlan_tci(),
        pkthdr.offvlan_tpid(),
        nethuns_vlan_tci(packet),
        nethuns_vlan_tpid(packet),
        nethuns_vlan_tci_(pkthdr, packet),
        nethuns_vlan_tpid_(pkthdr, packet),
        nethuns_vlan_vid(nethuns_vlan_tci_(pkthdr, packet)),
        pkthdr.rxhash()
    ));
    for byte in packet.iter().take(34) {
        builder.append(format!("{:02X} ", byte));
    }
    builder.append("\n");
    builder.string().unwrap()
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
