# rust-nethuns


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
