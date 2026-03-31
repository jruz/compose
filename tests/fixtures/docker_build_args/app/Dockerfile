FROM alpine:3.21
ARG GREETING
RUN echo "baked: ${GREETING}" > /greeting.txt
CMD ["cat", "/greeting.txt"]
