# =============================================================================
# AgentSync Dockerfile
# Multi-stage build for minimal, secure production image
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build environment
# Using official Rust image with Alpine for smaller base
# -----------------------------------------------------------------------------
FROM rust:1.85-alpine AS builder

# Build arguments for flexibility
ARG TARGETPLATFORM
ARG BUILDPLATFORM

WORKDIR /build

# Install build dependencies
# - musl-dev: Required for musl libc linking
# - pkgconfig: For finding system libraries
RUN apk add --no-cache \
    musl-dev \
    pkgconfig

# Copy only dependency files first for better layer caching
# This means dependencies are only rebuilt when Cargo.toml/Cargo.lock change
COPY Cargo.toml Cargo.lock ./

# Create a dummy project to build dependencies
# This exploits Docker's layer caching - dependencies change less often than source
RUN mkdir src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    echo 'pub fn dummy() {}' > src/lib.rs && \
    cargo build --release && \
    rm -rf src

# Copy the actual source code
COPY src ./src

# Touch main.rs to invalidate the dummy build, then build the real binary
# The dependencies are already cached from the previous step
RUN touch src/main.rs src/lib.rs && \
    cargo build --release --locked

# Verify the binary was built and strip debug symbols
RUN strip /build/target/release/agentsync && \
    /build/target/release/agentsync --version

# -----------------------------------------------------------------------------
# Stage 2: Runtime environment
# Minimal Alpine image - no Rust toolchain, no build dependencies
# -----------------------------------------------------------------------------
FROM alpine:3.21 AS runtime

# Labels for container metadata (OCI standard)
LABEL org.opencontainers.image.title="AgentSync" \
      org.opencontainers.image.description="Sync AI agent configurations across multiple AI coding assistants using symbolic links" \
      org.opencontainers.image.url="https://github.com/dallay/agentsync" \
      org.opencontainers.image.source="https://github.com/dallay/agentsync" \
      org.opencontainers.image.vendor="Yuniel Acosta" \
      org.opencontainers.image.licenses="MIT"

# Install runtime dependencies
# - ca-certificates: For HTTPS connections (future-proofing)
# - tini: Proper init system for containers (handles signals correctly)
RUN apk add --no-cache \
    ca-certificates \
    tini

# Create non-root user for security
# Running as root in containers is a security anti-pattern
RUN addgroup -g 1000 agentsync && \
    adduser -u 1000 -G agentsync -s /bin/sh -D agentsync

# Copy binary from builder stage
COPY --from=builder /build/target/release/agentsync /usr/local/bin/agentsync

# Ensure binary is executable
RUN chmod +x /usr/local/bin/agentsync

# Switch to non-root user
USER agentsync

# Set working directory for mounted volumes
WORKDIR /workspace

# Use tini as entrypoint for proper signal handling
# This ensures SIGTERM is properly forwarded to the process
ENTRYPOINT ["/sbin/tini", "--", "agentsync"]

# Default command shows help
CMD ["--help"]
