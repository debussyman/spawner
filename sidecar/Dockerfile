FROM rust:latest as build

WORKDIR /work
COPY . .
RUN cargo test --release
RUN cargo build --release

FROM gcr.io/distroless/cc-debian11

COPY --from=build /work/target/release/spawner-sidecar /spawner-sidecar
ENTRYPOINT [ "/spawner-sidecar" ]
