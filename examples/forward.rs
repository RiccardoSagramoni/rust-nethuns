use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{mem, thread};

use nethuns::sockets::errors::NethunsSendError;
use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use num_format::{Locale, ToFormattedString};


#[derive(Debug, Default)]
struct Configuration {
    dev_in: String,
    dev_out: String,
}


fn main() {
    let conf = get_configuration();
    
    let opt = NethunsSocketOptions {
        numblocks: 4,
        numpackets: 65536,
        packetsize: 2048,
        timeout_ms: 20,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    // Open sockets
    let in_socket = BindableNethunsSocket::open(opt.clone())
        .unwrap()
        .bind(&conf.dev_in, NethunsQueue::Any)
        .unwrap();
    let out_socket = BindableNethunsSocket::open(opt)
        .unwrap()
        .bind(&conf.dev_out, NethunsQueue::Any)
        .unwrap();
    
    
    let term = Arc::new(AtomicBool::new(false));
    let total_rcv = Arc::new(AtomicU64::new(0));
    let total_fwd = Arc::new(AtomicU64::new(0));
    
    let meter_th = {
        let total_rcv = total_rcv.clone();
        let total_fwd = total_fwd.clone();
        let term = term.clone();
        thread::spawn(move || {
            meter(total_rcv, total_fwd, term);
        })
    };
    
    // Set handler for Ctrl-C
    set_sigint_handler(term.clone());
    
    loop {
        // Check if Ctrl-C was pressed
        if term.load(Ordering::Relaxed) {
            break;
        }
        
        if let Ok(pkt) = in_socket.recv() {
            total_rcv.fetch_add(1, Ordering::SeqCst);
            loop {
                match out_socket.send(pkt.buffer()) {
                    Ok(_) => break,
                    Err(NethunsSendError::InUse) => {
                        out_socket.flush().expect("flush failed");
                    }
                    Err(e) => {
                        panic!("Error sending packet: {}", e);
                    }
                }
            }
            total_fwd.fetch_add(1, Ordering::SeqCst);
        }
    }
    
    meter_th.join().expect("meter_th join failed");
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
        let total_rcv = total_rcv.swap(0, Ordering::SeqCst);
        let total_fwd = total_fwd.swap(0, Ordering::SeqCst);
        println!(
            "pkt/sec: {} fwd/sec: {}",
            total_rcv.to_formatted_string(&Locale::en),
            total_fwd.to_formatted_string(&Locale::en),
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
        term.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");
}
