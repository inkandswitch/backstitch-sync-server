# Backstitch Sync Server

This is an Automerge-based TCP sync server, intended for use with the Godot version control plugin [Backstitch](https://backstitch.dev/).

Join our [Discord](https://discord.gg/SkW9vem5Ez) for support & community!

## Docker - Easiest Setup

An example Docker Compose configuration is available in `compose.example.yml`. It runs the image from the ghcr.io `backstitch-sync-server` package.

```sh
docker compose -f compose.example.yml up
```

See [`compose.example.yml`](./compose.example.yml) for details on configuration.

To connect a Backstitch client, enter the url `http://<ADDRESS>:<PORT>`. The port should be the port that maps to the HTTP port (`3000` in the Docker container), NOT the sync port (`8085`). If the mapped HTTP port is `80`, you don't need to specify the port.

## VPN Tunnel

By default, the server runs at localhost:PORT, and anyone on your local network will be able to access it. If you want to expose it to other team members, we **highly recommend** using a VPN tunneling service like Tailscale or ZeroTier.

Alternatively, you can directly port-forward with your server provider or home router. But since Backstitch Sync Server doesn't (yet) provide authentication, anyone who guesses your project ID will be able to access or edit your data.


## Building & Manual Installation

Clone this repository locally. To build and run, first, install [Rust and Cargo](https://rust-lang.org/tools/install/). Then, to install `just`, run:

```sh
cargo install just
```

## Usage

To build and run the server with defaults, use `just run`:

```sh
just run
```

For a list of potential configurable arguments:

```sh
just
```


Data will be stored to `./data` by default, but can be overridden.

The server will run a TCP `samod` connection at `localhost:8085`, as well as an HTTP server for server-description and testing at `localhost:3000`.

When you wish to connect with a Backstitch client, enter the `http_port` server URL. The default is `http://localhost:3000`.


## IMPORTANT: Security!

This server isn't set up for authentication. If someone guesses the ID of a project, they will be able to access all data associated with the project.

As such, before exposing this server to the internet, it is ***highly*** recommended to hide it behind a separate, secure VPN tunnel, or another method of connection authentication.
