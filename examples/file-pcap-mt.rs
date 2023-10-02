use std::{env, mem, thread};

use nethuns::sockets::base::RecvPacket;
use nethuns::sockets::base::pcap::{NethunsSocketPcap, NethunsSocketPcapTrait};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsSocketMode,
    NethunsSocketOptions,
};
use rtrb::{RingBuffer, Consumer};


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
    let (mut producer, mut consumer) = RingBuffer::<RecvPacket>::new(65536);
    
    thread::spawn(move || {
        consumer_body(consumer);
    });
    
}


fn get_target_filename() -> String {
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("usage {} file", args[0]);
    }
    mem::take(&mut args[1])
}


fn consumer_body(consumer: Consumer<RecvPacket>) {
    
}
