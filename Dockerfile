FROM debian:stable-slim
COPY rustgbot /usr/bin/rustgbot
ENTRYPOINT [ "/usr/bin/rustgbot" ]
