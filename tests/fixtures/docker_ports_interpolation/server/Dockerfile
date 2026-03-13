FROM alpine:3.21
RUN apk add --no-cache busybox-extras
RUN mkdir -p /www && echo "port-interpolation-works" > /www/index.html
CMD ["sh", "-c", "touch /tmp/ready && /usr/sbin/httpd -f -p 8080 -h /www"]
