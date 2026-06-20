# Backstitch Sync Server

This is an Automerge-based TCP sync server, intended for use with the Godot version control plugin [Backstitch](https://backstitch.dev/).

Join our [Discord](https://discord.gg/SkW9vem5Ez)!

## Installation

Clone this repository locally. To build and run, first, install [Rust and Cargo](https://rust-lang.org/tools/install/). Then, to install `just`, run:

```
cargo install just
```

## Usage

To build and run the server with defaults, use `just run`:

```
just run [data_dir] [port] [http_port] [debug|release]
```

Data will be stored to `./data` by default, but can be overridden.

The server will run a TCP `samod` connection at `localhost:8085`, as well as an HTTP server for testing at `localhost:3000`. 

## Docker

An example Docker Compose configuration is available in `compose.example.yml`. It runs the image from the ghcr.io `backstitch-sync-server` package.

```
docker compose -f compose.example.yml up
```

Or, to run the published image without Docker Compose:

```
docker run --rm \
  -p 8085:8085 \
  -p 3000:3000 \
  -v backstitch-data:/data \
  ghcr.io/inkandswitch/backstitch-sync-server:latest
```

The container uses these defaults:

| Variable | Default | Description |
| --- | --- | --- |
| `DATA_DIR` | `/data` | Directory where sync data is stored. Mount this as a volume for persistence. |
| `PORT` | `8085` | TCP `samod` sync server port. |
| `HTTP_PORT` | `3000` | HTTP server port for testing and document inspection. |


## IMPORTANT: Security!

This server isn't set up for authentication. If someone guesses the ID of a project, they will be able to access all data associated with the project.

As such, before exposing this server to the internet, it is ***highly*** recommended to hide it behind a separate, secure VPN tunnel, or another method of connection authentication.
