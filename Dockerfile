FROM rust:1.63-slim-bullseye AS rust-build
RUN apt-get update -y && apt-get install -y wget gcc build-essential cmake protobuf-compiler
ADD . /code
RUN cd /code && cargo build --release

FROM debian:bullseye-slim
WORKDIR /home
COPY --from=rust-build /code/target/release/irn .

EXPOSE 44668