mod constants;

use cfg_if::cfg_if;


cfg_if!(
    if #[cfg(feature="NETHUNS_USE_BUILTIN_PCAP_READER")] {
        mod builtin_reader;
        pub use builtin_reader::*;
    } else {
        mod pcap_reader;
        pub use pcap_reader::*;
    }
);
