FROM node:20 as node_builder
WORKDIR /app
COPY . .
RUN npm i -g pnpm
RUN pnpx tailwindcss -i game/app.css -o game/app.dist.css

FROM rust:1.77 as rust_builder
WORKDIR /app
COPY --from=node_builder /app .
RUN cargo build --release

FROM ubuntu:latest
WORKDIR /app
COPY --from=rust_builder /app/target/release/battleship .
COPY --from=node_builder /app/game game
ENV RUST_LOG=debug
ENTRYPOINT [ "./battleship" ]
