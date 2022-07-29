FROM rust:1.62.0-alpine as build

WORKDIR /src

RUN apk add --no-cache musl-dev

COPY . .

RUN cargo build --release

FROM scratch

COPY --from=build /src/target/release/cargo-install-from-manifest /bin/from-manifest

RUN ["/bin/from-manifest", "--help"]