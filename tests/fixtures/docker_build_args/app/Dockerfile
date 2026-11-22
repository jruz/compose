FROM alpine:3.21
ARG GREETING
ARG EXTRA
RUN printf 'baked: %s\nextra: %s\n' "${GREETING:-unset}" "${EXTRA:-unset}" > /greeting.txt
CMD ["cat", "/greeting.txt"]
