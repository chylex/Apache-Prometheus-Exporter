FROM rust:alpine as builder

RUN apk add --no-cache musl-dev

WORKDIR /app/
COPY . .

RUN cargo install --path .

FROM alpine
COPY --from=builder /usr/local/cargo/bin/apache_prometheus_exporter /usr/local/bin/apache_prometheus_exporter
CMD ["apache_prometheus_exporter"]
