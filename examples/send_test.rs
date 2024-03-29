use std::time::Duration;
use std::{env, thread};

use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{
    NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode,
    NethunsSocketOptions,
};

const PAYLOAD: [u8; 34] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
    0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
    0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
    0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
    0x07, 0x08,
];


fn main() {
    let opt = NethunsSocketOptions {
        numblocks: 4,
        numpackets: 10,
        packetsize: 2048,
        dir: NethunsCaptureDir::InOut,
        capture: NethunsCaptureMode::Default,
        mode: NethunsSocketMode::RxTx,
        promisc: false,
        rxhash: true,
        tx_qdisc_bypass: false,
        ..Default::default()
    };
    let socket: NethunsSocket = BindableNethunsSocket::open(opt)
        .expect("Failed to open socket")
        .bind(
            &env::args()
                .nth(1)
                .expect("Usage: ./send_test <device_name>"),
            NethunsQueue::Any,
        )
        .expect("Failed to bind socket");
    
    for i in 0..10 {
        for _ in 0..40 {
            socket.send(&PAYLOAD).expect("Failed to send packet");
        }
        socket.flush().expect("Failed to flush socket");
        println!("flush {i}");
        thread::sleep(Duration::from_secs(1));
    }
}
