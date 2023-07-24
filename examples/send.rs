#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use pico_args;
use std::error::Error;
use std::ffi::{c_int, c_uchar, CString};

const HELP_BRIEF: &str = "\
Usage:  nethuns-send [ options ]
Use --help (or -h) to see full option list and a complete description

Required options:
            [ -i <ifname> ]     set network interface
Other options:
            [ -b <batch_sz> ]   set batch size
            [ -n <nsock> ]      set number of sockets
            [ -m ]              enable multithreading
            [ -z ]              enable send zero-copy
";

const HELP_LONG: &str = "\
Usage:  nethuns-send [ options ]

-h, --help                      Show program usage and exit

Required options:

-i, --interface     <ifname>    Name of the network interface that nethuns-send operates on.

Other options:

-b, --batch_size    <batch_sz>  Batch size for packet transmission (default = 1).

-n, --sockets       <nsock>     Number of sockets to use. By default, only one socket is used.

-m, --multithreading            Enable multithreading. By default, only one thread is used.
                                If multithreading is enabled, and there is more than one socket in use,
                                each socket is handled by a separated thread.

-z, --zerocopy                  Enable send zero-copy. By default, classic send that requires a copy is used.
";

#[derive(Debug)]
struct Args {
    interface: CString,
    batch_size: c_int,
    num_sockets: c_int,
    multithreading: bool,
    zerocopy: bool,
}

fn main() {
    const payload: [c_uchar; 34] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
        0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
        0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
        0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
        0x07, 0x08,
    ];
    
    // Parse options from command line
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error in parsing command line options: {e}.");
            eprint!("{}", HELP_BRIEF);
            std::process::exit(0);
        }
    };
    
    println!("Args: {:#?}", args);
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut pargs = pico_args::Arguments::from_env();
    
    // Help has a higher priority and should be handled separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP_LONG);
        std::process::exit(0);
    }
    
    let args = Args {
        interface: CString::new::<String>(pargs.value_from_str(["-i", "--interface"])?)?,
        batch_size: pargs.value_from_str(["-b", "--batch_size"]).unwrap_or(1),
        num_sockets: pargs.value_from_str(["-n", "--sockets"]).unwrap_or(1),
        multithreading: pargs.contains(["-m", "--multithreading"]),
        zerocopy: pargs.contains(["-z", "--zerocopy"]),
    };
    
    // It's up to the caller what to do with the remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }
    
    Ok(args)
}
