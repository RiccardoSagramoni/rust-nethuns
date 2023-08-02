use nethuns::types::{NethunsCaptureDir, NethunsCaptureMode, NethunsQueue, NethunsSocketMode, NethunsSocketOptions};
use nethuns::NethunsSocketFactory;

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
    let mut socket = NethunsSocketFactory::try_new_nethuns_socket(opt).unwrap();
    socket.bind("vi0", NethunsQueue::Any).unwrap();
}
