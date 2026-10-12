# Stage 1: Build binary with Rust
FROM rust:bookworm AS builder

WORKDIR /usr/src/oxide-cms

# Cache dependencies
COPY Cargo.toml Cargo.lock ./

RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY src ./src

# Build release binary
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/oxide-cms/target/release/oxide-cms /app/oxide-cms

# Copy web templates and static assets
COPY templates ./templates
COPY static ./static

# Directory for persistent SQLite storage
RUN mkdir -p /data

ENV HOST=0.0.0.0
ENV PORT=3000
ENV DATABASE_URL=sqlite:/data/oxide_cms.db

EXPOSE 3000

VOLUME ["/data"]

CMD ["/app/oxide-cms"]
