fn main() {
    // Assert that only one feature flag have been enabled
    // for the underlying I/O framework
    assert_io_framework_mutual_exclusivity();
}

/// Check the feature flags for the underlying I/O frameworks.
///
/// # Panics
/// If none or more than one feature flag have been enabled for the underlying
/// I/O framework.
fn assert_io_framework_mutual_exclusivity() {
    #[allow(unused_mut)]
    let mut found: u8 = 0;
    
    cfg_if::cfg_if! {
        if #[cfg(feature="netmap")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="libpcap")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="xdp")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="tpacket_v3")] {
            found += 1;
        }
    };
    
    if found == 0 {
        panic!("Error: no I/O framework found. Enable one of the following features: netmap, libpcap, xdp, tpacket_v3.");
    }
    if found > 1 {
        panic!("Error: more than one I/O framework found. Enable only one of the following features: netmap, libpcap, xdp, tpacket_v3.");
    }
}
