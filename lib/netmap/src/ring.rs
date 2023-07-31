use crate::bindings::netmap_ring;

#[derive(Debug, Default)]
pub struct NetmapRing {
    pub r: Box<netmap_ring>,
}

impl From<*mut netmap_ring> for NetmapRing {
	fn from(ptr: *mut netmap_ring) -> Self {
		assert!(!ptr.is_null());
		Self {
			r: unsafe { Box::from_raw(ptr) },
		}
	}
}
