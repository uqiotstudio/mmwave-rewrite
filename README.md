# mmwave-rewrite
See the [quickstart](https://github.com/McArthur-Alford/mmwave-deploy/blob/main/README.md) guide if you are interested in a step-by-step for getting everything running. Below is some more general purpose documentation.

# Supported Devices:
At the moment the following modules are provided:
- AWR1843(AOP/Boost) devices (for the texas instruments AWR sensors)
- Zed 2i device (for the stereolabs Zed2i Camera)
- A file recorder for saving data

# Binaries:
All binaries support the argument `-t` and `-d` for detailed logging and debug information. It is recommended to run with `-t` to be notified of errors.

### mmwave-discovery
Opens the current device up for disocvery via mdns, without needing to manually set an IP address.
This should be run on the device that is hosting nats. This may not work on all networks.

```
Usage: mmwave-discovery [OPTIONS]

Options:
  -p, --port <PORT>  Port for server [default: 3000]
  -d, --debug        Enable debug logging
  -t, --tracing      Whether to use tracing
  -h, --help         Print help
  -V, --version      Print version
```

### mmwave-machine
This service should be run on each client machine. Each client machine should have a unique machine id,
each device in the configuration file specifies a machine and device id to inform the client which devices it should run.

```
Usage: mmwave-machine [OPTIONS] --machine-id <MACHINE_ID>

Options:
  -i, --ip <IP>                  IP address for server (ipv4)
  -p, --port <PORT>              Port for server [default: 3000]
  -m, --machine-id <MACHINE_ID>  Number of times to greet
  -d, --debug                    Enable debug logging
  -t, --tracing                  Whether to use tracing
  -h, --help                     Print help
  -V, --version                  Print version
```

### mmwave-dashboard
The dashboard. Allows creation/application of configurations, and visualisation of data. This is a GUI dashboard, not CLI.

```
Usage: mmwave-dashboard [OPTIONS]

Options:
  -i, --ip <IP>      IP address for server (ipv4)
  -p, --port <PORT>  Port for server [default: 3000]
  -d, --debug        Enable debug logging
  -t, --tracing      Whether to use tracing
  -h, --help         Print help
  -V, --version      Print version
```

# Usage
For easy deployment utilizing nix, see https://github.com/McArthur-Alford/mmwave-deploy

## Server
On the machine designated as the server (hosting NATs), run the following:
``mmwave-discovery -t``
``sh ./nats_server.sh``
For a port other than 3000, mmwave-discovery takes the port argument. nats_server.sh is fairly simple and easy to modify.

Assuming you have nats, cargo and all other dependencies installed the server should be up and running. Keep in mind that some networks don't support MDNS Discovery, in which case the discovery service is useless. Instead, find the server ip address and pass it as the --ip (-i) arg to mmwave-machine and dashboard on clients.

The server can be run on the same machine as a client with no issues.

## Configuration/Dashboard
An example configuration file should exist at ``<project_root_dir>/config_out.json``.

If the config file does not exist, or you want to utilize a new config, then on any machine, run the dashboard via ``cargo run --bin mmwave-dashboard -- -t``.
In the dashboard, on the right hand panel, add new devices. The config can be sent to NATS (and thus connected clients) via apply, and can be saved to config_out.json (overwriting it) with the save button.

If the config file exists, it can be manually loaded without the dashboard via the commands:
```sh
nats kv add config
nats kv put config config "$(cat ./config_out.json)"
```

## Client
On any client machine, run ``cargo run --bin mmwave-machine -- -m <machine-id> -t``. This will start a machine, which should wait until the server is found and then begin listening for any device configurations that match the machine id.

Device specific Details:
- For mmwave devices, the user running mmwave-machine must have permissions to read/write from `/dev/ttyACM*` for the boost and `/dev/ttyUSB*` for the aop.
- For the zed camera, a device with cuda must be utilized. Modify the command as follows: ``LD_LIBRARY_PATH=$LD_LIBRARY_PATH:<project_root_dir>/crates/mmwave-zed/cpp/build cargo run --features=zed_camera --bin mmwave-machine -- -m <machine-id> -t``.
