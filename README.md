# rust-nethuns

## WIP: files
```
├── api.h MERGED (in lib.rs for API and misc.rs for utility functions)
├── define.h MERGED (in vlan.rs)
├── global.c OK
├── global.h OK
├── misc
│   ├── compiler.h CAN'T
│   ├── hashmap.h NOT NECESSARY (use `std::collections::HashMap`)
│   └── macro.h OK
├── nethuns.c OK
├── nethuns.h OK
├── queue.h NOT NECESSARY (use `rtrb` crate)
├── sockets
│   ├── base.h OK
│   ├── file.inc MERGED (in pcap.rs)
│   ├── netmap.c OK
│   ├── netmap.h OK
│   ├── netmap_pkthdr.h OK
│   ├── ring.h OK
│   ├── types.h OK (in sockets.rs with traits)
├── stub.h OK (included in sockets.rs and vlan.rs)
├── types.h OK
├── version.c.in NOT NECESSARY (Rust already has a versioning system)
└── vlan.h OK
```

Unable to rewrite in Rust
- `compiler.h`



## WIP: Framework API (stub.h)

- [X] `nethuns_pcap_open(...)`
- [X] `nethuns_pcap_close(...)`
- [X] `nethuns_pcap_read(...)`
- [X] `nethuns_pcap_write(...)`
- [X] `nethuns_pcap_store(...)`
- [X] `nethuns_pcap_rewind(...)`

- [X] `nethuns_open(...)`
- [X] `nethuns_close(...)`
- [X] `nethuns_bind(...)`
- [X] `nethuns_fd(...)`
- [X] `nethuns_recv(...)`
- [X] `nethuns_flush(...)`
- [X] `nethuns_send(...)`
- [X] `nethuns_get_buf_addr(...)`
- [X] `nethuns_fanout(...)`

- [X] `nethuns_tstamp_sec(...)`
- [X] `nethuns_tstamp_usec(...)`
- [X] `nethuns_tstamp_nsec(...)`
- [X] `nethuns_tstamp_set_sec(...)`
- [X] `nethuns_tstamp_set_usec(...)`
- [X] `nethuns_tstamp_set_nsec(...)`

- [X] `nethuns_snaplen(...)`
- [X] `nethuns_len(...)`
- [X] `nethuns_set_snaplen(...)`
- [X] `nethuns_set_len(...)`
`
- [X] `nethuns_rxhash(...)`
- [X] `nethuns_dump_rings(...)`
- [X] `nethuns_stats(...)`

- [X] `nethuns_offvlan_tci(...)`
- [X] `nethuns_offvlan_tpid(...)`


## Examples

To load `/dev/netmap` and generate VALE ports:

```sh
sudo modprobe netmap

sudo vale-ctl -n vi0
sudo vale-ctl -a vale0:vi0
sudo vale-ctl -n vi1
sudo vale-ctl -a vale0:vi1
```


### send

PC1

```sh
./run_example.sh send "-i vi0 -b 64 -z"
```

PC2

```sh
sudo pkt-gen -i vi1 -f rx
```


### meter

PC1

```sh
pkt-gen -i vi0 -f tx
```

PC2

```sh
./run_example.sh meter "-i vi1"
```
