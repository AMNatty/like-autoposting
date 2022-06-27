FROM rust:latest as build

RUN update-ca-certificates

ENV USER=likeposter
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /likeposter

COPY ./ .

RUN cargo build --release

FROM debian:buster-slim

COPY --from=build /etc/passwd /etc/passwd
COPY --from=build /etc/group /etc/group

WORKDIR /likeposter

RUN chown -R 10001:10001 .

COPY --from=build /likeposter/target/release/likeposting ./

USER likeposter:likeposter

CMD ["/likeposter/likeposting"]