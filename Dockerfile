# SmartAssist Multi-Stage Dockerfile
# Builds the `smartassist` CLI binary and provides a minimal runtime image.
#
# Build:
#   docker build -t smartassist:latest .
#
# Run gateway:
#   docker run --rm -p 18789:18789 smartassist:latest gateway
#
# Run cron:
#   docker run --rm smartassist:latest cron
#
# Run doctor:
#   docker run --rm smartassist:latest doctor --full

# ---------------------------------------------------------------------------
# Builder stage
# ---------------------------------------------------------------------------
FROM rust:1.75-bookworm AS builder

WORKDIR /build

# Install system dependencies required by native crates
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy full source tree
COPY . .

# Build the CLI binary in release mode
RUN cargo build --release --bin smartassist

# ---------------------------------------------------------------------------
# Runtime stage
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 smartassist

WORKDIR /app
COPY --from=builder /build/target/release/smartassist /usr/local/bin/smartassist

USER smartassist

# Default health check (override in compose per service)
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD smartassist doctor || exit 1

ENTRYPOINT ["smartassist"]
CMD ["--help"]
