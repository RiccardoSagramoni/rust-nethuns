use std::env;

use etherparse::Ethernet2Header;
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns::vlan::{nethuns_vlan_tci_, nethuns_vlan_tpid_, nethuns_vlan_tci, nethuns_vlan_tpid, nethuns_vlan_vid};
use nethuns::{NethunsSocketFactory, RecvPacket};


fn main() {
    let opt = NethunsSocketOptions {
        numblocks: 4,
        numpackets: 10,
        packetsize: 2048,
        timeout_ms: 0,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: true,
        rxhash: true,
        tx_qdisc_bypass: false,
        ..Default::default()
    };
    let mut socket = NethunsSocketFactory::nethuns_socket_open(opt).unwrap();
    socket.bind(
        &env::args().nth(1).expect("Usage: ./recv_test <device_name>"), 
        NethunsQueue::Any
    ).unwrap();
    
    for _ in 0..5000 {
        match socket.recv() {
            Ok(p) => {
                dump_packet(&p);
            },
            Err(e) => {
                eprintln!("[ERROR]: {}", e);
            }
        }
    }
}


fn dump_packet(pkt: &RecvPacket) {
    print!(
        concat!(
            "{}:{} snap:{} len:{} offload{{tci:{:X} tpid:{:X}}} ",
            "packet{{tci:{:X} pid:{:X}}} => [tci:{:X} tpid:{:X} vid:{:X}] rxhash:0x{:X} | "
        ),
        pkt.pkthdr.tstamp_sec(), 
        pkt.pkthdr.tstamp_nsec(), 
        pkt.pkthdr.snaplen(), 
        pkt.pkthdr.len(), 
        pkt.pkthdr.offvlan_tci(), 
        pkt.pkthdr.offvlan_tpid(),
        nethuns_vlan_tci(pkt.packet),
        nethuns_vlan_tpid(pkt.packet),
        nethuns_vlan_tci_(pkt.pkthdr.as_ref(), pkt.packet),
        nethuns_vlan_tpid_(pkt.pkthdr.as_ref(), pkt.packet),
        nethuns_vlan_vid(nethuns_vlan_tci_(pkt.pkthdr.as_ref(), pkt.packet)),
        pkt.pkthdr.rxhash()
    );
    
    if let Ok(eth) = Ethernet2Header::from_slice(pkt.packet) {
        println!("{:?}", eth.0);
    }
}
