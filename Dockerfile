FROM rustserverless/lambda-rust:latest AS base

ARG TARGETARCH
ARG CARGO_LAMBDA_VERSION="v1.0.3"
ARG ZIG_VERSION="0.14.0"

RUN yum install -y gzip
RUN yum install -y tar
RUN yum install -y xz

ENV RUSTUP_HOME="/usr/local/rustup"
ENV CARGO_HOME="/usr/local/cargo"
ENV PATH="/usr/local/cargo/bin:${PATH}"
ENV PATH="/cargo/bin:/root/.wasmtime/bin:${PATH}"

# wasmtime
RUN case "${TARGETARCH}" in \
      amd64) wasmtime_arch="x86_64" ;; \
      arm64) wasmtime_arch="aarch64" ;; \
      *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
 && curl -LO "https://github.com/bytecodealliance/wasmtime/releases/download/v43.0.1/wasmtime-v43.0.1-${wasmtime_arch}-linux.tar.xz" \
 && tar -xf "wasmtime-v43.0.1-${wasmtime_arch}-linux.tar.xz" \
 && mv "wasmtime-v43.0.1-${wasmtime_arch}-linux/wasmtime" /usr/local/bin/ \
 && rm -rf "wasmtime-v43.0.1-${wasmtime_arch}-linux"*
RUN rustup default stable
RUN rustup target add wasm32-wasip1

# cargo lambda
RUN curl -fL "https://github.com/cargo-lambda/cargo-lambda/releases/download/${CARGO_LAMBDA_VERSION}/cargo-lambda-${CARGO_LAMBDA_VERSION}.x86_64-unknown-linux-musl.tar.gz" -o /tmp/cargo-lambda.tar.gz \
 && tar -xzf /tmp/cargo-lambda.tar.gz -C /tmp \
 && mv /tmp/cargo-lambda /cargo/bin/ \
 && rm /tmp/cargo-lambda.tar.gz

# zig
RUN case "${TARGETARCH}" in \
      amd64) zig_arch="x86_64" ;; \
      arm64) zig_arch="aarch64" ;; \
      *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
 && curl -fLO "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${zig_arch}-${ZIG_VERSION}.tar.xz" \
 && tar -xf "zig-linux-${zig_arch}-${ZIG_VERSION}.tar.xz" \
 && mv "zig-linux-${zig_arch}-${ZIG_VERSION}/zig" /usr/local/bin/ \
 && chmod 755 /usr/local/bin/zig \
 && rm -rf "zig-linux-${zig_arch}-${ZIG_VERSION}"*



FROM base AS builder

WORKDIR /app
COPY ./crates ./crates
COPY ./proto ./proto
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo lambda build --manifest-path ./crates/agent/Cargo.toml --release



FROM base AS build-agent

WORKDIR ${LAMBDA_RUNTIME_DIR}
COPY --from=builder /app/target/lambda/buildplane-agent/bootstrap .
COPY ./crates/exit_codes ./crates/exit_codes
COPY ./crates/runner ./crates/runner
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo lambda build --manifest-path ./crates/runner/Cargo.toml --target=wasm32-wasip1 --release

CMD ["bootstrap"]
