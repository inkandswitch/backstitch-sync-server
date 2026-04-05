# Backstitch Sync Server

This is an Automerge-based TCP sync server, intended for use with the Godot version control plugin [Backstitch](https://github.com/inkandswitch/patchwork-godot-plugin/).

## Installation

Clone this repository locally. To build and run, first, install [Rust and Cargo](https://rust-lang.org/tools/install/). Then, to install `just`, run:

```
cargo install just
```

## Usage

To build and run the server with defaults, use `just run`:

```
run [data_dir] [port] [http_port] [debug|release]
```

Data will be stored to `./data` by default, but can be overridden.

The server will run a TCP `samod` connection at `localhost:8085`, as well as an HTTP server for testing at `localhost:3000`. 

## IMPORTANT: Security!

This server isn't set up for authentication. If someone guesses the ID of a project, they will be able to access all data associated with the project.

As such, before exposing this server to the internet, it is ***highly*** recommended to hide it behind a separate, secure VPN tunnel, or another method of connection authentication.
