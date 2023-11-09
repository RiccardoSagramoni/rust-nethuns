use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{mem, thread};

use bus::{Bus, BusReader};
use nethuns::sockets::base::RecvPacket;
use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns_hybrid_rc::state::{Shared, Local};
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
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    // Open socket
    let socket: NethunsSocket<Shared> = BindableNethunsSocket::open(opt)
        .expect("Failed to open nethuns socket")
        .bind(&conf.dev, NethunsQueue::Any)
        .expect("Failed to bind nethuns socket");
    
    thread::scope(|s| {
        // Create SPSC ring buffer
        let (mut pkt_producer, pkt_consumer) =
            RingBuffer::<RecvPacket<NethunsSocket<Shared>, Shared>>::new(65536);
        
        // Create channel for thread communication
        let mut bus: Bus<()> = Bus::new(5);
        let total = Arc::new(AtomicU64::new(0));
        
        // Spawn meter thread
        let total1 = total.clone();
        let rx1 = bus.add_rx();
        s.spawn(move || {
            meter(total1, rx1);
        });
        
        // Spawn consumer thread
        let rx2 = bus.add_rx();
        s.spawn(move || {
            consumer_body(pkt_consumer, rx2, total);
        });
        
        // Set handler for Ctrl-C
        let mut bus_rx = bus.add_rx();
        set_sigint_handler(bus);
        
        loop {
            // Check if Ctrl-C was pressed
            match bus_rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => break,
                _ => {}
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


fn meter(total: Arc<AtomicU64>, mut rx: BusReader<()>) {
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
fn set_sigint_handler(mut bus: Bus<()>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        bus.broadcast(());
    })
    .expect("Error setting Ctrl-C handler");
}


fn consumer_body(
    mut consumer: Consumer<RecvPacket<NethunsSocket<Shared>, Shared>>,
    mut rx: BusReader<()>,
    total: Arc<AtomicU64>,
) {
    loop {
        match rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => (),
        }
        
        // Read packet
        if consumer.pop().is_ok() {
            total.fetch_add(1, Ordering::AcqRel);
        }
    }
}
