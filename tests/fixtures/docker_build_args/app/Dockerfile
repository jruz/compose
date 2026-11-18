FROM alpine:3.21
ARG GREETING
RUN echo "baked: ${GREETING:-unset}" > /greeting.txt
CMD ["cat", "/greeting.txt"]
