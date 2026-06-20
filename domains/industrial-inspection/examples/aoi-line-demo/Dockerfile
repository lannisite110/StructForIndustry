FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y capnproto \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release -p sfi-core-bus -p sfi-plugin-host \
    && cargo build --release --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml \
    && cargo build --release --manifest-path domains/industrial-inspection/plugins/mes-reporter/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/sfi-bus /usr/local/bin/
COPY --from=builder /app/target/release/sfi-mock-defect-detect /usr/local/bin/
COPY --from=builder /app/target/release/sfi-mock-ai-infer /usr/local/bin/
COPY --from=builder /app/domains/industrial-inspection/hal-ext/line-publisher/target/release/sfi-line-publisher /usr/local/bin/
COPY --from=builder /app/domains/industrial-inspection/plugins/mes-reporter/target/release/sfi-mes-reporter /usr/local/bin/
COPY domains/industrial-inspection /app/domains/industrial-inspection
COPY domains/industrial-inspection/examples/aoi-line-demo/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENV SFI_DATA_DIR=/var/lib/sfi
ENV SFI_BUS_HTTP=0.0.0.0:8080
VOLUME ["/var/lib/sfi"]
EXPOSE 8080
ENTRYPOINT ["/entrypoint.sh"]
