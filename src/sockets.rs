use core::fmt::Debug;

use crate::types::{NethunsQueue, NethunsSocketOptions};

use self::{errors::{NethunsBindError, NethunsOpenError}, base::NethunsSocketBase};

pub mod base;
pub mod errors;
pub mod ring;
pub mod ring_slot;
pub mod types;


// Import the structs defined for the required I/O framework
cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        mod netmap;
        
        use netmap::NethunsSocketNetmap;
        
        pub use netmap::Pkthdr;
    }
    else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}


/// Trait which defines the public API for Nethuns sockets.
pub trait NethunsSocket: Debug {
    fn try_new(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn NethunsSocket>, NethunsOpenError>
    where
        Self: Sized;
    
    
    fn bind(
        &mut self,
        dev: &str,
        queue: NethunsQueue,
    ) -> Result<(), NethunsBindError>;
    
    fn recv(&self) -> Result<(), String>; // FIXME better error type
    
    fn get_socket_base(&mut self) -> &mut NethunsSocketBase;
}


/// Factory to build objects which implements the trait NethunsSocket
pub struct NethunsSocketFactory();

impl NethunsSocketFactory {
    pub fn try_new_nethuns_socket(
        opt: NethunsSocketOptions,
    ) -> Result<Box<dyn NethunsSocket>, NethunsOpenError> {
        cfg_if::cfg_if! {
            if #[cfg(feature="netmap")] {
                return NethunsSocketNetmap::try_new(opt);
            }
            else {
                std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
            }
        }
    }
}
