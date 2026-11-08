# syntax=docker/dockerfile:1
FROM rust:1.98-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY crates ./crates
COPY src ./src
COPY migrations ./migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release && mkdir -p /out && cp target/release/vindictus-server /out/vindictus-server

# Admin panel single-page app (web/admin), served by the laboratory frontend's admin listener.
FROM node:24.11.1-bookworm-slim@sha256:48abc13a19400ca3985071e287bd405a1d99306770eb81d61202fb6b65cf0b57 AS admin-ui
WORKDIR /ui
COPY web/admin/package.json web/admin/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/admin/ ./
RUN npm run build

# Client builder (docs/future/game-launcher.md 4.10): the server binary, Python and the client recipe
# (scripts/client, scripts/content). It builds the server's own client tree from the imported 1.69 original
# and applies the published panel content to it, so no version needs the original on the operator's
# machine. Same Debian release as the runtime (the binary links the same glibc). Not the default target.
FROM python:3.13.18-slim-bookworm@sha256:a5a7017efc5969e00ae2b91c8eb0631049a569652aa4ffc4501769049ae45c72 AS client-builder
ARG RECIPE_COMMIT=""
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && pip install --no-cache-dir --disable-pip-version-check capstone==5.0.6 pillow==12.2.0 \
    && groupadd --gid 10001 vindictus \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin vindictus
WORKDIR /app
COPY --link --from=builder --chown=10001:10001 /out/vindictus-server /app/vindictus-server
# Only the recipe's sources and data: the vendored Windows capstone (.deps) and caches stay out (.dockerignore).
COPY --link --chown=10001:10001 scripts/client /app/recipe/scripts/client
COPY --link --chown=10001:10001 scripts/content /app/recipe/scripts/content
ENV VINDICTUS_RECIPE_COMMIT=${RECIPE_COMMIT} PYTHONDONTWRITEBYTECODE=1
USER 10001:10001
ENTRYPOINT ["/app/vindictus-server"]
CMD ["client-builder", "--help"]

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
COPY --link --from=admin-ui --chown=10001:10001 /ui/dist /app/admin-ui
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/app/vindictus-server"]
CMD ["serve"]

