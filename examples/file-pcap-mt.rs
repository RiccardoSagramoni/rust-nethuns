use std::sync::mpsc;
use std::{env, mem, thread};

use nethuns::sockets::base::pcap::{NethunsSocketPcap, NethunsSocketPcapTrait};
use nethuns::sockets::base::RecvPacket;
use nethuns::sockets::errors::NethunsPcapReadError;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsSocketMode,
    NethunsSocketOptions,
};
use rtrb::{Consumer, RingBuffer};


fn main() {
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
    
    // Open socket
    let mut socket: NethunsSocketPcap =
        NethunsSocketPcap::open(opt, get_target_filename().as_str(), false)
            .expect("unable to open `output` socket");
    
    
    // Create SPSC ring buffer
    let (mut producer, consumer) = RingBuffer::<RecvPacket>::new(65536);
    
    // Create channel for thread communication
    let (tx, rx) = mpsc::channel::<()>();
    
    // Spawn consumer thread
    let consumer_th = thread::spawn(move || {
        consumer_body(consumer, rx);
    });
    
    
    loop {
        match socket.read() {
            Ok(packet) => {
                while !producer.is_abandoned() {
                    if !producer.is_full() {
                        producer.push(packet).unwrap();
                        break;
                    }
                }
            }
            Err(NethunsPcapReadError::Eof) => {
                break;
            }
            Err(e) => {
                panic!("error: {:?}", e);
            }
        }
    }
    
    println!(
        "head: {}\n",
        socket.base().rx_ring().as_ref().unwrap().head()
    );
    tx.send(()).expect("unable to send signal in mpsc channel");
    consumer_th.join().expect("unable to join consumer thread");
}


fn get_target_filename() -> String {
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("usage {} file", args[0]);
    }
    mem::take(&mut args[1])
}


fn consumer_body(mut consumer: Consumer<RecvPacket>, rx: mpsc::Receiver<()>) {
    loop {
        // Read packet
        if let Ok(packet) = consumer.pop() {
            println!("packet: {}\n", packet);
        }
        
        if consumer.is_empty() && rx.try_recv().is_ok() {
            return;
        }
    }
}
