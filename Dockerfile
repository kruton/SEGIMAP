# Dockerfile for creating a statically-linked Rust application using docker's
# multi-stage build feature. This also leverages the docker build cache to avoid
# re-downloading dependencies if they have not changed.
FROM ekidd/rust-musl-builder AS build

# Download the target for static linking.
RUN rustup target add x86_64-unknown-linux-musl

RUN sudo chown -R rust:rust /home

# Install and cache dependencies layers
# Rather than copying everything every time, re-use cached dependency layers
# to install/build deps only when Cargo.* files change.
RUN USER=root cargo new /home/segimap --bin

WORKDIR /home/segimap
RUN mkdir -p core mime

# Download the dependencies so we don't have to do this every time.
COPY mime/Cargo.toml mime/Cargo.lock ./mime/
COPY core/Cargo.toml core/Cargo.lock ./core/

RUN touch mime/dummy.rs && \
    echo "fn main() {}" > core/dummy.rs && \
    sed -i 's#src/lib.rs#dummy.rs#' mime/Cargo.toml && \
    sed -i 's#src/main.rs#dummy.rs#' core/Cargo.toml && \
    cargo build --release --manifest-path core/Cargo.toml && \
    sed -i 's#dummy.rs#src/lib.rs#' mime/Cargo.toml && \
    sed -i 's#dummy.rs#src/main.rs#' core/Cargo.toml && \
    rm core/dummy.rs mime/dummy.rs

# Copy the source and build the application.
COPY mime/src ./mime/src/
COPY core/src ./core/src/

RUN cd core && cargo build --bins --release --target x86_64-unknown-linux-musl
RUN find /home/segimap/ -name segimap

# Copy the statically-linked binary into a scratch container.
FROM scratch
LABEL org.opencontainers.image.source https://github.com/kruton/segimap
COPY --from=build /home/segimap/core/target/x86_64-unknown-linux-musl/release/segimap ./
USER 1000
ENV RUST_LOG=debug
EXPOSE 3000/tcp
EXPOSE 10000/tcp
EXPOSE 10001/tcp
ENTRYPOINT ["./segimap"]
