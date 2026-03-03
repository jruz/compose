FROM alpine:3.21
CMD ["sh", "-c", "echo 'api listening on :8080' && trap 'echo shutting down; exit 0' TERM && while true; do sleep 1; done"]
