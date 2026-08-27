# Backstitch Sync Server

This is an Automerge-based Websockets sync server, intended for use with the Godot version control plugin [Backstitch](https://backstitch.dev/).

Join our [Discord](https://discord.gg/SkW9vem5Ez) for support & community!

## Docker: Basic Configuration

A simple Docker Compose configuration is available in [`examples/docker-compose.simple.yml`](./examples/docker-compose.yml). It runs the image from the ghcr.io `backstitch-sync-server` package.

```sh
docker compose -f docker-compose.simple.yml up
```

See [`examples/docker-compose.simple.yml`](./examples/docker-compose.simple.yml) for details on configuration.

By default, the server runs at `http://localhost`, and anyone on your local network will be able to access it. If you want to expose it to other team members, we **highly recommend** using a VPN tunneling service like Tailscale or ZeroTier.

## Docker: Advanced Configuration


### The Backstitch Webviewer

Adding the [Backstitch Webviewer](https://github.com/inkandswitch/backstitch-webviewer) allows the server to play WebGL-compatible games directly in the browser, right from Backstitch! All without exporting your game. 

This is great for students who want to easily share their games with the class, or developers who want to send their friends or teammates links to playtest online.

To include the Webviewer in the Docker configuration, you'll need to include an extra `backstitch-webviewer` image. Please see [`examples/docker-compose.webviewer.yml`](./examples/docker-compose.webviewer.yml) for a basic configuration. This example will host the Webviewer at the root of the server.

If you want to host the Webviewer separately, simply point the Backstitch API URL at your server. To enable the "Copy Playable Link" button in the Backstitch client, set the `WEBVIEWER` environment variable to the desired Webviewer URL.

### HTTPS with Caddy

If you have a domain name for your server (like `alpha.backstitch.dev`), we highly recommend setting up HTTPS certificates. You can do so by putting a `caddy` reverse proxy in front of your Backstitch Sync Server image.

This is easy to do with Docker as well. For an example of how to add Caddy to your setup, see [`examples/docker-compose.caddy.yml`](./examples/docker-compose.caddy.yml).

### OpenID Connect Authentication
Backstitch allows optional authentication through OpenID Connect. OpenID Connect is a protocol based on OAuth 2.0 that many identity providers use, like Google or Okta, or many private organizations. Setting up OpenID Connect would allow you to log into Backstitch using your Google/Okta/etc credentials!

For a Docker example, see [`examples/docker-compose.oidc.yml`](./examples/docker-compose.yml).

However, setup can be fairly complicated, and is primarily intended for organizations. Most independent developers should **still be using ZeroTier or Tailscale** to expose their server to the internet.

**Warning:** Backstitch Sync Server currently provides OpenID Connect *authentication*, but not *authorization.* If you use Google as your OpenID Connect provider, for example, *anyone* with a Google account could log into Backstitch. 

In the future, we will provide an endpoint to hook into the Backstitch Sync Server and add custom authorization -- like user whitelisting or domain restriction. For now, make sure you run the OpenID Connect provider in question, or are OK with public access.


## IMPORTANT: Security!

By default, this server isn't set up for authentication. If someone guesses the ID of a project, they will be able to access all data associated with the project.

As such, before exposing this server to the internet, it is ***highly*** recommended to hide it behind a separate, secure VPN tunnel, or another method of connection authentication.

## For Developers: Building & Installation

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

The parameters of the server mirror those environment variables found in the Docker configuration.

There are a few useful recipes in the Justfile, as well:

```sh
just dev            # Uses a local ./data directory at port 3000, creating it if it doesn't exist
just dev-oidc       # Does the same, but also spins up a Rauthy OpenID Connect instance for developers.
                    # Requires Docker to be installed and running.
just dev-endless    # Does the same, but instead uses Endless Access's OpenID Connect configuration.
```


## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md)
