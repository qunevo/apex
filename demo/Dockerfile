FROM python:3.13-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends gosu \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 1000 demo \
    && useradd --uid 1000 --gid demo --no-create-home demo
WORKDIR /opt
COPY . /opt/demo
RUN chmod +x /opt/demo/scripts/container.sh
EXPOSE 8788
ENTRYPOINT ["/opt/demo/scripts/container.sh"]
