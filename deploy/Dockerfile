# syntax=docker/dockerfile:1
FROM node:lts-trixie-slim AS node
FROM rust:slim-trixie AS source
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates pkg-config libssl-dev cmake && rm -rf /var/lib/apt/lists/*
ENV CARGO_BUILD_JOBS=2
WORKDIR /work
COPY . .

FROM source AS server-build
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/work/target \
    cargo build --release --locked -p liar-server --bins && \
    mkdir /binaries && cp target/release/liar-server target/release/migrate target/release/metrics /binaries/

FROM source AS web-build
COPY --from=node /usr/local/ /usr/local/
RUN npm install --global corepack && corepack yarn install --immutable
RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install wasm-bindgen-cli --locked
ARG VITE_PUBLIC_SITE_ORIGIN
ARG VITE_ADSENSE_CLIENT_ID
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/work/target \
    corepack yarn build && node deploy/render-nginx.mjs /work/nginx-routes.conf

FROM debian:trixie-slim AS server
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && \
    useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin liar
WORKDIR /app
COPY --from=server-build /binaries/ /app/bin/
COPY migrations/ /app/migrations/
COPY --chmod=0555 deploy/entrypoint.sh /app/entrypoint.sh
ENV LIAR_BIND=0.0.0.0:3000 LIAR_MIGRATIONS=/app/migrations
USER 10001:10001
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=15s --timeout=3s --start-period=30s --retries=3 CMD ["curl", "--fail", "--silent", "--max-time", "2", "http://127.0.0.1:3000/health/live"]
ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["serve"]

FROM nginx/nginx-unprivileged:stable-alpine AS web
COPY --from=web-build /work/apps/web/dist/ /usr/share/nginx/html/
COPY --from=web-build /work/nginx-routes.conf /etc/nginx/routes.conf
COPY deploy/nginx.conf /etc/nginx/nginx.conf
USER 101:101
STOPSIGNAL SIGQUIT
HEALTHCHECK --interval=15s --timeout=3s --retries=3 CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/health/live"]
ENTRYPOINT ["nginx", "-c", "/etc/nginx/nginx.conf", "-g", "daemon off;"]
