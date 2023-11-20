use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{mem, thread};

use nethuns::sockets::{BindableNethunsSocket, RecvPacket};

use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};
use rtrb::{Consumer, RingBuffer};


#[derive(Debug, Default)]
struct Configuration {
    dev_in: String,
    dev_out: String,
}


fn main() {
    let conf = get_configuration();
    
    let opt = NethunsSocketOptions {
        numblocks: 1,
        numpackets: 1024,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: false,
        ..Default::default()
    };
    
    let socket = BindableNethunsSocket::open(opt.clone())
        .unwrap()
        .bind(&conf.dev_in, NethunsQueue::Any)
        .unwrap();
    
    
    thread::scope(|s| {
        // Create SPSC ring buffer
        let (mut producer, consumer) = RingBuffer::<RecvPacket>::new(65536);
        
        // Create channel for thread communication
        let term = Arc::new(AtomicBool::new(false));
        let total_rcv = Arc::new(AtomicU64::new(0));
        let total_fwd = Arc::new(AtomicU64::new(0));
        
        // Spawn meter thread
        let meter_total_rcv = total_rcv.clone();
        let meter_total_fwd = total_fwd.clone();
        let meter_term = term.clone();
        s.spawn(move || {
            meter(meter_total_rcv, meter_total_fwd, meter_term);
        });
        
        // Spawn consumer thread
        let consumer_opt = opt;
        let consumer_dev = conf.dev_out.clone();
        let consumer_term = term.clone();
        s.spawn(move || {
            consumer_body(
                consumer_opt,
                &consumer_dev,
                consumer,
                consumer_term,
                total_fwd,
            );
        });
        
        // Set handler for Ctrl-C
        set_sigint_handler(term.clone());
        
        
        loop {
            // Check if Ctrl-C was pressed
            if term.load(Ordering::Relaxed) {
                break;
            }
            
            if let Ok(pkt) = socket.recv() {
                total_rcv.fetch_add(1, Ordering::AcqRel);
                // Push packet in queue
                while !producer.is_abandoned() {
                    if !producer.is_full() {
                        producer.push(pkt).unwrap();
                        break;
                    }
                }
            }
        }
    });
}


fn get_configuration() -> Configuration {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!("Usage: {} <device_in> <device_out>", args[0]);
    }
    Configuration {
        dev_in: mem::take(&mut args[1]),
        dev_out: mem::take(&mut args[2]),
    }
}


fn meter(
    total_rcv: Arc<AtomicU64>,
    total_fwd: Arc<AtomicU64>,
    term: Arc<AtomicBool>,
) {
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
        let total_rcv = total_rcv.swap(0, Ordering::AcqRel);
        let total_fwd = total_fwd.swap(0, Ordering::AcqRel);
        println!(
            "pkt/sec: {} fwd/sec: {} ",
            total_rcv.to_formatted_string(&Locale::en),
            total_fwd.to_formatted_string(&Locale::en)
        );
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
        term.store(true, Ordering::Relaxed)
    })
    .expect("Error setting Ctrl-C handler");
}


fn consumer_body(
    opt: NethunsSocketOptions,
    dev: &str,
    mut consumer: Consumer<RecvPacket>,
    term: Arc<AtomicBool>,
    total_fwd: Arc<AtomicU64>,
) {
    let socket = BindableNethunsSocket::open(opt)
        .unwrap()
        .bind(dev, NethunsQueue::Any)
        .unwrap();
    
    loop {
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        // Read packet
        if let Ok(pkt) = consumer.pop() {
            loop {
                match socket.send(pkt.buffer()) {
                    Ok(_) => break,
                    Err(_) => {
                        socket.flush().unwrap();
                    }
                }
            }
            total_fwd.fetch_add(1, Ordering::AcqRel);
        }
    }
}
