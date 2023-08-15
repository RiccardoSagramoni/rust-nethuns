#!/bin/sh

# Load netmap module
sudo modprobe netmap

# Generate some VALVE ports for example testing
sudo vale-ctl -n vi0
sudo vale-ctl -a vale0:vi0
sudo vale-ctl -n vi1
sudo vale-ctl -a vale0:vi1
