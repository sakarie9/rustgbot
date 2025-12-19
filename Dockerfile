FROM debian:stable-slim
ARG TARGETPLATFORM
COPY $TARGETPLATFORM/rustgbot /usr/bin/rustgbot
ENTRYPOINT [ "/usr/bin/rustgbot" ]
