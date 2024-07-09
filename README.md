# mmwave-rewrite

This project provides helpful tools for networked recording (utilizing NATS) of various sensors utilized in research under Dr Matthew D'Souza and Dr Bronwyn Clark for classification of human motion.

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
The dashboard. Allows creation/application of configurations, and visualisation of data.
This is not a CLI dashboard, you need a gui.

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
## Server
On the server, run the following binaries:
``cargo run --bin mmwave-discovery``

## Configuration

## Client
