use nethuns::sockets::{BindableNethunsSocket, NethunsSocket};
use nethuns::types::{NethunsQueue, NethunsSocketOptions};

fn main() {
    let opt = NethunsSocketOptions::default();
    
    let socket: NethunsSocket = BindableNethunsSocket::open(opt)
        .unwrap()
        .bind("dev", NethunsQueue::Any)
        .unwrap();
    
    let p = socket.recv().unwrap();
    drop(socket);
    drop(p);
}
