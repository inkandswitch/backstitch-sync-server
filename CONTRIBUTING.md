
# Backstitch Sync Server: Contributor's Guide

We'd love your help developing the Backstitch Sync Server. Here's how to get started.

## Prerequisites

First, install Rust and Cargo. Then install `just` with `cargo install just`. 

## Basic Development

Use `just dev` to run a basic development server. The data directory is automatically set to `./data`. 

# OpenID Connect Development

Run `just oidc-dev` to spin up a local authentication server with Rauthy, as well as running the server. You must have `docker` and `docker-compose` installed to run the test authentication server.