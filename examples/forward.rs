use std::ops::DerefMut;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{mem, thread};

use bus::{Bus, BusReader};
use nethuns::sockets::nethuns_socket_open;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};


#[derive(Debug, Default)]
struct Configuration {
    dev_in: String,
    dev_out: String,
}


fn main() {
    let conf = parse_args();
    
    let opt = NethunsSocketOptions {
        numblocks: 4,
        numpackets: 2048,
        packetsize: 2048,
        timeout_ms: 20,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: true,
        rxhash: false,
        tx_qdisc_bypass: true,
        ..Default::default()
    };
    
    // Open sockets
    let mut in_socket = nethuns_socket_open(opt.clone())
        .unwrap()
        .bind(&conf.dev_in, NethunsQueue::Any)
        .unwrap();
    let mut out_socket = nethuns_socket_open(opt)
        .unwrap()
        .bind(&conf.dev_out, NethunsQueue::Any)
        .unwrap();
    
    
    let mut bus: Bus<()> = Bus::new(5);
    let total_rcv = Arc::new(Mutex::new(0_u64));
    let total_fwd = Arc::new(Mutex::new(0_u64));
    
    let meter_th = {
        let total_rcv = total_rcv.clone();
        let total_fwd = total_fwd.clone();
        let rx = bus.add_rx();
        thread::spawn(move || {
            meter(total_rcv, total_fwd, rx);
        })
    };
    
    // Set handler for Ctrl-C
    let mut bus_rx = bus.add_rx();
    set_sigint_handler(bus);
    
    loop {
        // Check if Ctrl-C was pressed
        match bus_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            _ => {}
        }
        
        if let Ok(pkt) = in_socket.recv() {
            *(total_rcv.lock().expect("lock failed")) += 1;
            loop {
                match out_socket.send(pkt.packet().borrow_packet()) {
                    Ok(_) => break,
                    Err(_) => {
                        out_socket.flush().unwrap();
                    }
                }
            }
            *(total_fwd.lock().expect("lock failed")) += 1;
        }
    }
    
    meter_th.join().expect("join failed");
}


fn parse_args() -> Configuration {
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
    total_rcv: Arc<Mutex<u64>>,
    total_fwd: Arc<Mutex<u64>>,
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
        
        // Print statistics
        let total_rcv = mem::replace(total_rcv.lock().unwrap().deref_mut(), 0);
        let total_fwd = mem::replace(total_fwd.lock().unwrap().deref_mut(), 0);
        println!("pkt/sec: {total_rcv} fwd/sec: {total_fwd} ");
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
