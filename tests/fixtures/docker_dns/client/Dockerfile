FROM alpine:3.21
CMD ["sh", "-c", "ping -c 1 -W 3 dns-server > /dev/null 2>&1 && echo dns-resolved || echo dns-failed"]
