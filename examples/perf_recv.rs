use std::sync::mpsc::{self, Sender, TryRecvError};
use std::time::{Duration, SystemTime};
use std::{env, mem, thread};

use nethuns::sockets::errors::NethunsRecvError;
use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};


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
    
    
    // Define channel for MPSC communication between threads
    let (sigint_sender, sigint_receiver) = mpsc::channel::<()>();
    
    // Set timer for stopping data collection after 10 minutes
    let _ = {
        let sigint_sender = sigint_sender.clone();
        let stop_time = SystemTime::now()
            .checked_add(Duration::from_secs(10 * 60))
            .unwrap();
        thread::spawn(move || {
            if let Ok(delay) = stop_time.duration_since(SystemTime::now()) {
                thread::sleep(delay);
            }
            sigint_sender.send(()).unwrap();
        })
    };
    // Set handler for Ctrl-C
    set_sigint_handler(sigint_sender);
    
    
    // Start receiving
    let mut total: u64 = 0;
    let mut time_for_logging = SystemTime::now()
        .checked_add(Duration::from_secs(1))
        .unwrap();
    
    loop {
        // Check condition for program termination
        match sigint_receiver.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        if time_for_logging < SystemTime::now() {
            let total = mem::replace(&mut total, 0);
            println!("{total}");
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
fn set_sigint_handler(sender: Sender<()>) {
    ctrlc::set_handler(move || {
        println!("Ctrl-C detected. Shutting down...");
        sender.send(()).unwrap();
    })
    .expect("Error setting Ctrl-C handler");
}
