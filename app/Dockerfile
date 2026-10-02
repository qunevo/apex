# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS build
WORKDIR /source
COPY . .
RUN cargo build --locked --release -p apex-control-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash ca-certificates curl openssl postgresql-client \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 apex \
    && useradd --uid 10001 --gid apex --no-create-home apex \
    && mkdir -p /var/lib/apex /run/apex-bootstrap \
    && chown apex:apex /var/lib/apex /run/apex-bootstrap
WORKDIR /opt/apex
COPY --from=build /source/target/release/apex-control /usr/local/bin/apex-control
COPY customization ./customization
COPY skills ./skills
COPY LICENSE LICENSING.md ./
COPY --chmod=755 scripts/container.sh /usr/local/bin/apex-container
ENV APEX_CONTROL_AUTH=/var/lib/apex/identity/auth.json \
    APEX_CONTROL_BIND=0.0.0.0:8780
USER apex
EXPOSE 8780
ENTRYPOINT ["apex-container"]
CMD ["serve"]
