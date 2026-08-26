FROM rust:1-trixie AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:13-slim AS runtime

RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin backstitch \
    && mkdir -p /data \
    && chown -R backstitch:backstitch /data

COPY --from=builder /app/target/release/backstitch-sync-server /usr/local/bin/backstitch-sync-server

COPY docker/entrypoint.sh /usr/local/bin/backstitch-entrypoint.sh
RUN chmod +x /usr/local/bin/backstitch-entrypoint.sh

USER backstitch

ENV RUST_LOG=info,samod=info,samod_core=info
ENV RUST_BACKTRACE=1

VOLUME ["/data"]

EXPOSE 3000/tcp

ENTRYPOINT ["/usr/local/bin/backstitch-entrypoint.sh"]