#include <nethuns/nethuns.h>
#include <stdio.h>
#include <getopt.h>
#include <signal.h>

#include <atomic>
#include <thread>
#include <chrono>
#include <iostream>
#include <string>
#include <vector>


/// manage command line options
const struct option long_opts[] = {
        {"help", no_argument, 0, 'h'},
        {"interface", required_argument, 0, 'i'},
        {"batch_size", required_argument, 0, 'b'},
        {"sockets", required_argument, 0, 'n'},
        {"multithreading", no_argument, 0, 'm'},
        {"zerocopy", no_argument, 0, 'z'},
        {0, 0, 0, 0}
};

const std::string help_brief = "Usage:  nethuns-send [ options ]\n" \
                                "Use --help (or -h) to see full option list and a complete description.\n\n"
                                "Required options: \n" \
                                "\t\t\t[ -i <ifname> ] \t set network interface \n" \
                                "Other options: \n" \
                                "\t\t\t[ -b <batch_sz> ] \t set batch size \n" \
                                "\t\t\t[ -n <nsock> ] \t\t set number of sockets \n" \
                                "\t\t\t[ -m ] \t\t\t enable multithreading \n" \
                                "\t\t\t[ -z ] \t\t\t enable send zero-copy \n";

// nethuns socket
nethuns_socket_t* out = new nethuns_socket_t();
struct nethuns_socket_options netopt;
char* errbufs = new char[NETHUNS_ERRBUF_SIZE];

// configuration
uint64_t pktid = 0;
std::string interface = "";
int batch_size = 1;
bool zerocopy = false;

// stats collection
uint64_t total = 0;
#define     METER_DURATION_SECS    10 * 60 + 1
#define     METER_RATE_SECS        10


// terminate application
std::atomic<bool> term(false);

// termination signal handler
void terminate(int exit_signal)
{
    (void)exit_signal;
    term.store(true, std::memory_order_relaxed);
}

void terminate_program(std::chrono::system_clock::time_point stop_timestamp) {
    std::this_thread::sleep_until(stop_timestamp);
    term.store(true, std::memory_order_relaxed);
}


// setup and fill transmission ring
inline void fill_tx_ring(const unsigned char *payload, int pkt_size)
{
    unsigned int j;

    out = nethuns_open(&netopt, errbufs);
    if (!out) {
        throw std::runtime_error(errbufs);
    }
    
    if (nethuns_bind(out, interface.c_str(), NETHUNS_ANY_QUEUE) < 0) {
        throw nethuns_exception(out);
    }

    // fill the slots in the tx ring (optimized send only)
    if (zerocopy) {
        for (j = 0; j < nethuns_txring_get_size(out); j++) {
            uint8_t *pkt = nethuns_get_buf_addr(out, j);    // tell me where to copy the j-th packet to be transmitted
            memcpy(pkt, payload, pkt_size);                         // copy the packet
        }
        pktid = 0;                                          // first position (slot) in tx ring to be transmitted
    }
}

// transmit packets in the tx ring (use optimized send, zero copy)
inline void transmit_zc(int pkt_size)
{
    // prepare batch
    for (int n = 0; n < batch_size; n++) {
        if (nethuns_send_slot(out, pktid, pkt_size) <= 0) {
            break;
        }
        pktid++;
        total++;
    }
    nethuns_flush(out);             // send batch
}

// transmit packets in the tx ring (use classic send, copy)
inline void transmit_c(const unsigned char *payload, int pkt_size)
{
    // prepare batch
    for (int n = 0; n < batch_size; n++) {
        if (nethuns_send(out, payload, pkt_size) <= 0) {
            break;
        }
        total++;
    }
    nethuns_flush(out);             // send batch
}

inline std::chrono::system_clock::time_point next_meter_log() {
    return std::chrono::system_clock::now() + std::chrono::seconds(METER_RATE_SECS);
}


int main(int argc, char *argv[])
{
    static const unsigned char payload[34] =
    {
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf0, 0xbf, /* L`..UF.. */
        0x97, 0xe2, 0xff, 0xae, 0x08, 0x00, 0x45, 0x00, /* ......E. */
        0x00, 0x54, 0xb3, 0xf9, 0x40, 0x00, 0x40, 0x11, /* .T..@.@. */
        0xf5, 0x32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, /* .2...... */
        0x07, 0x08
    };

    // parse options from command line
    int opt = 0;
    int optidx = 0;
    opterr = 1;     // turn on/off getopt error messages
    if (argc > 1 && argc < 10) {
        while ((opt = getopt_long(argc, argv, "hi:b:n:mz", long_opts, &optidx)) != -1) {
            switch (opt) {
            case 'h':
                std::cout << help_brief << std::endl;
                return 0;
            case 'i':
                if (optarg)
                    interface = optarg;
                break;
            case 'b':
                if (optarg)
                    batch_size = atoi(optarg);
                break;
            case 'z':
                zerocopy = true;
                break;
            default:
                std::cerr << "Error in parsing command line options.\n" << help_brief << std::endl;
                return 1;
            }
        }
    } else {
        std::cerr << help_brief << std::endl;
        return 1;
    }

    std::cout << "\nTest " << argv[0] << " started with parameters \n"
                            << "* interface: " << interface << " \n"
                            << "* batch_size: " << batch_size << " \n"
                            << "* zero-copy: " << ((zerocopy) ? " ON \n" : " OFF \n")
                            << std::endl;
    
    signal(SIGINT, terminate);  // register termination signal

    // nethuns options
    netopt = {
        .numblocks       = 1
    ,   .numpackets      = 2048
    ,   .packetsize      = 2048
    ,   .timeout_ms      = 0
    ,   .dir             = nethuns_in_out
    ,   .capture         = nethuns_cap_zero_copy
    ,   .mode            = nethuns_socket_rx_tx
    ,   .promisc         = false
    ,   .rxhash          = false
    ,   .tx_qdisc_bypass = true
    ,   .xdp_prog        = nullptr
    ,   .xdp_prog_sec    = nullptr
    ,   .xsk_map_name    = nullptr
    ,   .reuse_maps      = false
    ,   .pin_dir         = nullptr
    };
       
    // Init nethuns socket
    fill_tx_ring(payload, 34);
        
    // set up timer for stopping data collection after 10 minutes
    std::thread stop_th(
        terminate_program, 
        std::chrono::system_clock::now() + std::chrono::seconds(METER_DURATION_SECS)
    );
    
     
    try {
        auto time_to_log = next_meter_log();
        
        while (!term.load(std::memory_order_relaxed)) {
            // print stats every second
            if (time_to_log < std::chrono::system_clock::now()) {
                std::cout << total << std::endl;
                total = 0;
                time_to_log = next_meter_log();
            }
            
            if (zerocopy) {
                transmit_zc(34);
            }
            else {
                transmit_c(payload, 34);
            }
        }
    } catch(nethuns_exception &e) {
        if (e.sock) {
            nethuns_close(e.sock);
        }
        std::cerr << e.what() << std::endl;
        return 1;
    } catch(std::exception &e) {
        std::cerr << e.what() << std::endl;
        return 1;
    }
    
    nethuns_close(out);
    return 0;
}
