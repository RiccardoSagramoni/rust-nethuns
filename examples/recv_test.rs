
use std::env;

use etherparse::Ethernet2Header;
use nethuns::sockets::base::RecvPacket;
use nethuns::sockets::{BindableNethunsSocket, NethunsSocket, Local};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns::vlan::{
    nethuns_vlan_tci, nethuns_vlan_tci_, nethuns_vlan_tpid, nethuns_vlan_tpid_,
    nethuns_vlan_vid,
};


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
    let socket: NethunsSocket<Local> = BindableNethunsSocket::open(opt)
        .expect("Failed to open socket")
        .bind(
            &env::args()
                .nth(1)
                .expect("Usage: ./recv_test <device_name>"),
            NethunsQueue::Any,
        )
        .expect("Failed to bind socket");
    
    loop {
        if let Ok(p) = socket.recv() {
            dump_packet(&p);
        }
    }
}


fn dump_packet(pkt: &RecvPacket<NethunsSocket<Local>, Local>) {
    let pkthdr = pkt.pkthdr();
    let packet = pkt.buffer();
    
    print!(
        concat!(
            "{}:{} snap:{} len:{} offload{{tci:{:X} tpid:{:X}}} ",
            "packet{{tci:{:X} pid:{:X}}} => [tci:{:X} tpid:{:X} vid:{:X}] rxhash:0x{:X} | "
        ),
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
    );
    
    if let Ok(eth) = Ethernet2Header::from_slice(packet) {
        println!("{:?}", eth.0);
    }
}
