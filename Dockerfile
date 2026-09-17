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
COPY desktop/Cargo.toml ./desktop/Cargo.toml
RUN mkdir -p src desktop/src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && echo 'fn main() {}' > desktop/src/main.rs \
    && echo 'fn main() {}' > desktop/build.rs \
    && cargo build --release --locked -p harmony \
    && rm -rf src

COPY src ./src
COPY --from=web /web/dist ./web/dist
RUN touch src/main.rs src/lib.rs && cargo build --release --locked -p harmony

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home harmony \
    && mkdir -p /data && chown harmony:harmony /data
COPY --from=build /app/target/release/harmony /usr/local/bin/harmony
USER harmony
ENV HARMONY_BIND=0.0.0.0:8080 \
    RUST_LOG=harmony=info
# Mount point for `HARMONY_STORAGE=file:/data`; unused with the Azure backend.
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["harmony"]
