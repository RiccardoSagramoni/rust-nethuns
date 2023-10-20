use nethuns::sockets::nethuns_socket_open;
use nethuns::types::{NethunsQueue, NethunsSocketOptions};

fn main() {
    let opt = NethunsSocketOptions::default();
    
    let socket = nethuns_socket_open(opt)
        .unwrap()
        .bind("dev", NethunsQueue::Any)
        .unwrap();
    
    let p = socket.recv().unwrap();
    drop(socket);
    drop(p);
}
