FROM rust as build-env
WORKDIR /app
ADD . /app
RUN cargo build --release

FROM gcr.io/distroless/cc-debian10:nonroot
USER nonroot
COPY --from=build-env /app/target/release/cs-tb-fix /
CMD ["/cs-tb-fix"]
