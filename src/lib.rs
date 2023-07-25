#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ptr;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

unsafe impl Send for nethuns_socket_options {}
unsafe impl Sync for nethuns_socket_options {}

fn hello_world () {
	println!("Hello, world!");
	unsafe {
		nethuns_close_netmap(ptr::null_mut());
	}
}
