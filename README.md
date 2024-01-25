# RustNethuns: a rewrite in Rust of the Nethuns unified API for fast and portable network programming

RustNethuns is a rewrite in Rust of [Nethuns](https://github.com/larthia/nethuns), a fast C-based network I/O library.
The aim of this work has been to evaluate the *high performance* and *strong safety* claims made by the Rust programming language, specifically in the domain of low-level network programming.

This project serves as the central element of Riccardo Sagramoni's MSc thesis in Computer Engineering.

## Related resources

- Final thesis document ([GitHub](https://github.com/RiccardoSagramoni/rust-nethuns-thesis))
- Performance evaluation of the RustNethuns library ([GitHub](https://github.com/RiccardoSagramoni/rust-nethuns-performance-analysis))
- Safety analysis of the RustNethuns’s socket model with the Miri interpreter ([GitHub](https://github.com/RiccardoSagramoni/rust-nethuns-miri))


## What is Nethuns?

Nethuns is a software library (originally written in C) that provides a unified API to access and manage low-level network operations over different underlying network I/O frameworks, and consequently operating systems.
The design of Nethuns originates from the practical requirement of developing portable network applications with extremely high data rate target.
Instead of re-writing the applications to match the underlying network I/O engines available over the different operating systems, Nethuns offers a unified abstraction layer that allows programmers to implement their applications regardless of the underlying technology.
Therefore, network applications that use the Nethuns library only need to be re-compiled to run on top of a different engine (chosen in the set of the ones available for the OS), with no need for code adaptation.

Nethuns would like to fill the lack of a unified network abstraction in the software domain, which is instead present in the hardware domain thanks to [P4](https://p4.org/).
Nethuns should play a similar role to that entrusted to the [pcap](https://www.tcpdump.org/) library in the past.
In addition, it adds support for recent technologies such as [AF_XDP](https://www.kernel.org/doc/Documentation/networking/af_xdp.rst) and concurrency.
Of course, all of this is provided to network programmers while minimizing the overhead, in order not to impact the performance of native underlying network I/O frameworks.
The API exposed by Nethuns recalls the interface of UNIX sockets to make immediate and simple its adoption to experienced network programmers.

Currently, the C-based Nethuns library fully supports:

- AF_PACKET and [AF_XDP](https://www.kernel.org/doc/Documentation/networking/af_xdp.rst) sockets for fast packet handling over Linux
- the [netmap](https://github.com/luigirizzo/netmap) framework for fast packet I/O over Linux and FreeBSD
- the [pcap](https://www.tcpdump.org/) library for use in BSD, MacOS and Windows operating systems


## Why a Rust-based Nethuns library?
...


## RustNethuns API
...


## Dependencies

The RustNethuns library relies on the following dependencies:

- **rustc** compiler.
- [**libclang**](https://clang.llvm.org/doxygen/group__CINDEX.html) library with *Clang 5.0 or greater*, needed to automatically generate the bindings to the underlying C-based I/O frameworks.
- [**netmap**](https://github.com/luigirizzo/netmap) library, needed to enable netmap support.


## Cargo features
...


## Using the library to implement a brand new application
...


## Credits

### Author

- Riccardo Sagramoni

### Supervisors

- Prof. Giuseppe Lettieri
- Prof. Gregorio Procissi

### Others

The [Lartia group](https://larthia.com/) for the original C-based [Nethuns](https://github.com/larthia/nethuns) library.
