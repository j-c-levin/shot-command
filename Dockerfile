# Multi-stage build for the headless game server.
# Produces a small image (~50-100MB) for Edgegap deployment.

# --- Builder stage ---
FROM rustlang/rust:nightly AS builder

RUN rustup component add rust-src

WORKDIR /app
COPY . .

# Override .cargo/config.toml linker settings — Docker image doesn't have clang/mold.
# Keep build-std and share-generics, just swap the linker to default gcc.
RUN mkdir -p /app/.cargo && \
    cat > /app/.cargo/config.toml <<'EOF'
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-Zshare-generics=y",
    "-Zthreads=0",
]

[unstable]
build-std = ["std", "core", "alloc", "panic_abort"]

[profile.release]
lto = "thin"
opt-level = "s"
strip = true
codegen-units = 1
EOF

RUN cargo build --release --bin server --target x86_64-unknown-linux-gnu

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/server /usr/local/bin/nebulous-server
COPY assets/maps/ /app/assets/maps/
WORKDIR /app

# Edgegap injects ARBITRIUM_PORT_GAMEPORT_INTERNAL at runtime.
# The server binary reads it from the environment automatically.
ENTRYPOINT ["/usr/local/bin/nebulous-server", "--bind", "0.0.0.0:5000"]
