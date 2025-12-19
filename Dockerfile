FROM debian:stable-slim
ARG TARGETPLATFORM
COPY rustgbot /usr/bin/rustgbot
ENTRYPOINT [ "/usr/bin/rustgbot" ]
