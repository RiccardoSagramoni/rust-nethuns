pub mod bindings;
pub mod constants;
pub mod macros;
pub mod nmport;
pub mod ring;
pub mod slot;


/// TODO
pub fn nm_pkt_copy(src: &[u8], dst: *mut libc::c_void) {
    unsafe {
        crate::bindings::nm_pkt_copy(
            src.as_ptr() as *const libc::c_void,
            dst,
            src.len() as libc::c_int,
        );
    }
    todo!();
}
