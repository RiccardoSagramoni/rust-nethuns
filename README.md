# rust-nethuns

## WIP: files
```
├── api.h OK
├── define.h OK
├── global.c OK
├── global.h OK
├── misc
│   ├── compiler.h CAN'T
│   ├── hashmap.h NOT NECESSARY
│   └── macro.h OK
├── nethuns.c OK
├── nethuns.h OK
├── queue.h TODO? <==
├── sockets
│   ├── base.h DOING (only pcap_* structs TODO) <==
│   ├── file.inc TODO <==
│   ├── netmap.c DONE
│   ├── netmap.h DOING (only pcap_* functions TODO) <==
│   ├── netmap_pkthdr.h OK
│   ├── ring.h DOING <==<==
│   ├── types.h OK (in sockets.rs with traits)
├── stub.h DONE (included in sockets.rs and vlan.rs)
├── types.h OK
├── version.c.in TODO?? <==
└── vlan.h OK
```

## WIP: Framework API (stub.h)

- [ ] `nethuns_pcap_open(...)`
- [ ] `nethuns_pcap_close(...)`
- [ ] `nethuns_pcap_read(...)`
- [ ] `nethuns_pcap_write(...)`
- [ ] `nethuns_pcap_store(...)`
- [ ] `nethuns_pcap_rewind(...)`

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
