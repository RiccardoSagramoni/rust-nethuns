use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{mem, thread};

use nethuns::sockets::base::RecvPacket;
use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};
use rtrb::{Consumer, RingBuffer};


#[derive(Debug, Default)]
struct Configuration {
    dev: String,
}


fn main() {
    let conf = get_configuration();
    
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 2048,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    // Open socket
    let socket: NethunsSocket = BindableNethunsSocket::open(opt)
        .expect("Failed to open nethuns socket")
        .bind(&conf.dev, NethunsQueue::Any)
        .expect("Failed to bind nethuns socket");
    
    thread::scope(|s| {
        // Create SPSC ring buffer
        let (mut pkt_producer, pkt_consumer) =
            RingBuffer::<RecvPacket>::new(65536);
        
        let term = Arc::new(AtomicBool::new(false));
        let total = Arc::new(AtomicU64::new(0));
        
        // Spawn meter thread
        let _ = {
            let total = total.clone();
            let term = term.clone();
            s.spawn(move || {
                meter(total, term);
            })
        };
        
        
        // Spawn consumer thread
        let _ = {
            let term = term.clone();
            s.spawn(move || {
                consumer_body(pkt_consumer, term, total);
            })
        };
        
        // Set handler for Ctrl-C
        set_sigint_handler(term.clone());
        
        loop {
            // Check if Ctrl-C was pressed
            if term.load(Ordering::Relaxed) {
                break;
            }
            
            if let Ok(pkt) = socket.recv() {
                // Push packet in queue
                while !pkt_producer.is_abandoned() {
                    if !pkt_producer.is_full() {
                        pkt_producer.push(pkt).unwrap();
                        break;
                    }
                }
            }
        }
    });
}


fn get_configuration() -> Configuration {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        panic!("Usage: {} <device>", args[0]);
    }
    Configuration {
        dev: mem::take(&mut args[1]),
    }
}


fn meter(total: Arc<AtomicU64>, term: Arc<AtomicBool>) {
    let mut now = SystemTime::now();
    
    loop {
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        // Sleep for 1 second
        let next_sys_time = now
            .checked_add(Duration::from_secs(1))
            .expect("SystemTime::checked_add() failed");
        if let Ok(delay) = next_sys_time.duration_since(now) {
            thread::sleep(delay);
        }
        now = next_sys_time;
        
        // Print statistics
        let total = total.swap(0, Ordering::AcqRel);
        println!("pkt/sec: {}", total.to_formatted_string(&Locale::en));
    }
}

/// Set an handler for the SIGINT signal (Ctrl-C),
/// which will notify the other threads
/// to gracefully stop their execution.
///
/// # Arguments
/// - `bus`: Bus for SPMC (single-producer/multiple-consumers) communication
///   between threads.
fn set_sigint_handler(term: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        term.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");
}


fn consumer_body(
    mut consumer: Consumer<RecvPacket>,
    term: Arc<AtomicBool>,
    total: Arc<AtomicU64>,
) {
    loop {
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        // Read packet
        if consumer.pop().is_ok() {
            total.fetch_add(1, Ordering::AcqRel);
        }
    }
}
