FROM rust:trixie as builder

# Make a fake Rust app to keep a cached layer of compiled crates
RUN USER=root cargo new app
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
# Needs at least a main.rs file with a main function
RUN mkdir src && echo "fn main(){}" > src/main.rs
# Will build all dependent crates in release mode
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo build --release

# Copy the rest
COPY . .
# Fetch the latest GitHub GraphQL schema
RUN wget -O ./res/graphql/github-schema.graphql \
    https://raw.githubusercontent.com/octokit/graphql-schema/master/schema.graphql
# Build (install) the actual binaries
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    cargo install --path .

# Runtime image
FROM debian:trixie-slim

# Install certificates so we can make outbound TLS connections
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/local/cargo/bin/str4d-fly-dev /app/str4d-fly-dev

# No CMD or ENTRYPOINT, see fly.toml with `cmd` override.
