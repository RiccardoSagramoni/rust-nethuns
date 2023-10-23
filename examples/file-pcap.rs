use std::{env, mem};

use nethuns::sockets::errors::NethunsPcapReadError;
use nethuns::sockets::pcap::NethunsSocketPcap;
use nethuns::sockets::{BindableNethunsSocket, PkthdrTrait};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};
use nethuns::vlan;
use num_format::{Locale, ToFormattedString};


#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
enum PcapMode {
    Read,
    Count,
    Capture,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
struct Configuration {
    mode: PcapMode,
    target_name: String,
}


fn main() {
    let conf = parse_args();
    
    match &conf.mode {
        PcapMode::Read | PcapMode::Count => run_read_count_mode(conf),
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


fn run_read_count_mode(conf: Configuration) {
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
    
    let socket: NethunsSocketPcap =
        NethunsSocketPcap::open(opt, &conf.target_name, false)
            .expect("unable to open `output` socket");
    
    let mut total: u64 = 0;
    let mut errors: u64 = 0;
    loop {
        match socket.read() {
            Ok(pkt) => {
                total += 1;
                
                if conf.mode == PcapMode::Count {
                    if total % 1_000_000 == 0 {
                        println!(
                            "packet: {}",
                            total.to_formatted_string(&Locale::en)
                        );
                    }
                } else {
                    let pkthdr = pkt.pkthdr();
                    println!(
                        "{}:{} caplen:{} len:{}: PACKET!",
                        pkthdr.tstamp_sec(),
                        pkthdr.tstamp_nsec(),
                        pkthdr.snaplen(),
                        pkthdr.len()
                    )
                }
            }
            Err(NethunsPcapReadError::Eof) => {
                break;
            }
            Err(e) => {
                errors += 1;
                eprintln!("Error: {e}");
                if errors % 1_000_000 == 0 {
                    eprintln!("errors: {}", total);
                }
            }
        }
    }
    
    println!("total packet: {}", total.to_formatted_string(&Locale::en));
    println!("total errors: {}", errors.to_formatted_string(&Locale::en));
    println!(
        "total       : {}",
        (total + errors).to_formatted_string(&Locale::en)
    );
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
    
    let out_socket = NethunsSocketPcap::open(
        opt.clone(),
        format!("{}.pcap", &conf.target_name).as_str(),
        true,
    )
    .expect("unable to open `output` socket");
    
    let in_socket = BindableNethunsSocket::open(opt)
        .expect("unable to open `input` socket")
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
            println!("{}", dump_packet(pkt.pkthdr(), pkt.buffer()));
            out_socket
                .store(pkt.pkthdr(), pkt.buffer())
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
        vlan::nethuns_vlan_tci(packet),
        vlan::nethuns_vlan_tpid(packet),
        vlan::nethuns_vlan_tci_(pkthdr, packet),
        vlan::nethuns_vlan_tpid_(pkthdr, packet),
        vlan::nethuns_vlan_vid(vlan::nethuns_vlan_tci_(pkthdr, packet)),
        pkthdr.rxhash()
    ));
    for byte in packet.iter().take(14) {
        builder.append(format!("{:02X} ", byte));
    }
    builder.append("\n");
    builder.string().unwrap()
}
