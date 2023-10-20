use nethuns::sockets::BindableNethunsSocket;
use nethuns::types::{NethunsQueue, NethunsSocketOptions};

fn main() {
    let opt = NethunsSocketOptions::default();
    
    let socket = BindableNethunsSocket::open(opt)
        .unwrap()
        .bind("dev", NethunsQueue::Any)
        .unwrap();
    
    let p = socket.recv().unwrap();
    drop(socket);
    drop(p);
}
