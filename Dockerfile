# ---- frontend --------------------------------------------------------------
FROM node:24-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-fund --no-audit
COPY web/ ./
RUN npm run build

# ---- backend ---------------------------------------------------------------
FROM rust:1.95-bookworm AS build
WORKDIR /app

# Build dependencies once against stub sources so they cache independently
# of the application code.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY --from=web /web/dist ./web/dist
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home harmony
COPY --from=build /app/target/release/harmony /usr/local/bin/harmony
USER harmony
ENV HARMONY_BIND=0.0.0.0:8080 \
    RUST_LOG=harmony=info
EXPOSE 8080
ENTRYPOINT ["harmony"]
