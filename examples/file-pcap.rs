use std::{env, mem};

use nethuns::sockets::base::pcap::{NethunsSocketPcap, NethunsSocketPcapTrait};
use nethuns::sockets::{nethuns_socket_open, PkthdrTrait};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns::vlan::{
    nethuns_vlan_tci, nethuns_vlan_tci_, nethuns_vlan_tpid, nethuns_vlan_tpid_,
    nethuns_vlan_vid,
};


#[derive(Debug, Copy, Clone)]
enum PcapMode {
    Read,
    Count,
    Capture,
}

#[derive(Debug, Clone)]
struct Configuration {
    mode: PcapMode,
    target_name: String,
}


fn main() {
    let conf = parse_args();

    match &conf.mode {
        PcapMode::Read | PcapMode::Count => {
            todo!()
        }
        PcapMode::Capture => run_capture_mode(conf),
    }
}


fn parse_args() -> Configuration {
    let mut args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        panic!(
            "usage {} [read filepcap | count filepcap | capture ifname]",
            args[0]
        );
    }

    let mode = match args[1].as_str() {
        "read" => PcapMode::Read,
        "count" => PcapMode::Count,
        "capture" => PcapMode::Capture,
        _ => panic!(
            "usage {} [read filepcap | count filepcap | capture ifname]",
            args[0]
        ),
    };

    Configuration {
        mode,
        target_name: mem::take(&mut args[2]),
    }
}


fn run_capture_mode(conf: Configuration) {
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

    let mut out_socket = NethunsSocketPcap::open(
        opt.clone(),
        format!("{}.pcap", &conf.target_name).as_str(),
        true,
    )
    .expect("unable to open `output` socket");

    let in_socket =
        nethuns_socket_open(opt).expect("unable to open `input` socket");
    let mut in_socket = in_socket
        .bind(&conf.target_name, NethunsQueue::Any)
        .unwrap_or_else(|_| {
            panic!(
                "unable to bind `input` socket to device {}",
                &conf.target_name
            )
        });
    
    let mut i = 0;
    while i < 10 {
        if let Ok(pkt) = in_socket.recv() {
            println!(
                "{}",
                dump_packet(
                    pkt.pkthdr().as_ref(),
                    pkt.packet().borrow_packet()
                )
            );
            out_socket
                .store(pkt.pkthdr().as_ref(), pkt.packet().borrow_packet())
                .expect("pcap store failed");
            i += 1;
        }
    }
}


fn dump_packet(pkthdr: &dyn PkthdrTrait, packet: &[u8]) -> String {
    let mut builder = string_builder::Builder::new(1024);
    builder.append(format!(
        "{}:{} snap:{} len:{} offload{{tci:{:X} tpid:{:X}}} packet{{tci:{:X} pid:{:X}}} => [tci:{:X} tpid:{:X} vid:{:X}] rxhash:0x{:X} | ",
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
    ));
    for byte in packet.iter().take(14) {
        builder.append(format!("{:02X} ", byte));
    }
    builder.append("\n");
    builder.string().unwrap()
}
