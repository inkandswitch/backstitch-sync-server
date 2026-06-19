FROM rust:1-trixie AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:13-slim AS runtime

RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin backstitch \
    && mkdir -p /data \
    && chown -R backstitch:backstitch /data

COPY --from=builder /app/target/release/server /usr/local/bin/backstitch-sync-server

USER backstitch

ENV DATA_DIR=/data
ENV PORT=8085
ENV HTTP_PORT=3000
ENV RUST_LOG=info,samod=info,samod_core=info

VOLUME ["/data"]

EXPOSE 8085/tcp
EXPOSE 3000/tcp

ENTRYPOINT ["backstitch-sync-server"]
