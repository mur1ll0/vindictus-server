# syntax=docker/dockerfile:1
FROM rust:1.98-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release && mkdir -p /out && cp target/release/vindictus-server /out/vindictus-server

FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587 AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 vindictus \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin vindictus
LABEL org.opencontainers.image.title="Vindictus preservation foundation" \
      org.opencontainers.image.description="Operational Rust foundation; legacy game migration incomplete" \
      org.opencontainers.image.version="0.1.0"
WORKDIR /app
COPY --link --from=builder --chown=10001:10001 /out/vindictus-server /app/vindictus-server
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/app/vindictus-server"]
CMD ["serve"]

