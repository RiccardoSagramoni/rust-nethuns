use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{NethunsQueue, NethunsSocketOptions};

fn main() {
    let opt = NethunsSocketOptions::default();
    
    let socket: NethunsSocket = BindableNethunsSocket::open(opt)
        .unwrap()
        .bind("dev", NethunsQueue::Any)
        .unwrap();
    
    let packet = socket.recv().unwrap();
    let buffer = packet.buffer();
    drop(socket);
    drop(buffer);
}
