# Backstitch Sync Server

This is an Automerge-based TCP sync server, intended for use with the Godot version control plugin [Backstitch](https://backstitch.dev/).

Join our [Discord](https://discord.gg/SkW9vem5Ez) for support & community!

## Docker - Easiest Setup

An example Docker Compose configuration is available in [`examples/docker-compose.yml`](./examples/docker-compose.yml). It runs the image from the ghcr.io `backstitch-sync-server` package.

```sh
docker compose -f compose.example.yml up
```

See [`examples/docker-compose.yml`](./examples/docker-compose.yml) for details on configuration.

If you already have an OpenID Connect authentication server for SSO, check out [`examples/docker-compose.oidc.yml`](./examples/docker-compose.oidc.yml) to set it up.

## Authentication

By default, the server runs at localhost:PORT, and anyone on your local network will be able to access it. If you want to expose it to other team members, we **highly recommend** using a VPN tunneling service like Tailscale or ZeroTier.

Alternatively, you can directly port-forward with your server provider or home router. But since Backstitch Sync Server doesn't (yet) provide authentication, anyone who guesses your project ID will be able to access or edit your data.

For advanced authentication with OpenID Connect, see the example in [`examples/docker-compose.yml`](./examples/docker-compose.basic.yml)


## Building & Manual Installation

Clone this repository locally. To build and run, first, install [Rust and Cargo](https://rust-lang.org/tools/install/). Then, to install `just`, run:

```sh
cargo install just
```

To build and run the production server with defaults, use `just run`:

```sh
just run
```

The server's REST API will run at `http://localhost:3000`. For a list of potential arguments:

```sh
just help
```


## IMPORTANT: Security!

This server isn't set up for authentication. If someone guesses the ID of a project, they will be able to access all data associated with the project.

As such, before exposing this server to the internet, it is ***highly*** recommended to hide it behind a separate, secure VPN tunnel, or another method of connection authentication.


## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md)
