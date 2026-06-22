FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
ARG TARGETPLATFORM
COPY $TARGETPLATFORM/rustgbot /usr/bin/rustgbot
ENTRYPOINT [ "/usr/bin/rustgbot" ]
