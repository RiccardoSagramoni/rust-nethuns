use nethuns::sockets::base::pcap::NethunsSocketPcap;
use nethuns::types::NethunsSocketOptions;

#[test]
fn main() {
    let opt = NethunsSocketOptions::default();
    
    let socket = NethunsSocketPcap::open(opt, "filename", false).unwrap();
    
    let p1 = socket.read().unwrap();
    let p2 = socket.read().unwrap();
    println!("{:?} {:?}", p1, p2);
    drop(socket);
    drop(p1);
}
