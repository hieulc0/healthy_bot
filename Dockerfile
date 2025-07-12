# Healthy Bot Dockerfile for Pixel 3a/postmarketOS + Discord
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev iproute2
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine
RUN apk add --no-cache iproute2
COPY --from=builder /app/target/release/healthy_bot /usr/local/bin/healthy_bot
CMD ["healthy_bot"]

