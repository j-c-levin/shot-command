# Multi-stage build for the headless game server.
# Produces a small image for Edgegap deployment.

# --- Builder stage ---
FROM rustlang/rust:nightly AS builder

# Dev libs needed at compile time (linked statically with --no-default-features)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libwayland-dev libxkbcommon-dev libasound2-dev libudev-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rust-src

WORKDIR /app
COPY . .

# Override .cargo/config.toml — Docker image doesn't have clang/mold.
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

RUN cargo build --release --no-default-features --features release --bin server --target x86_64-unknown-linux-gnu

# --- Runtime stage ---
# Slim image — binary only needs libc, libm, libgcc_s (no wayland/alsa/udev)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/server /usr/local/bin/nebulous-server
COPY assets/maps/ /app/assets/maps/
WORKDIR /app

ENTRYPOINT ["/usr/local/bin/nebulous-server", "--bind", "0.0.0.0:5000"]
