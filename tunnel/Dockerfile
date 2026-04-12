FROM cloudflare/cloudflared:latest AS cloudflared
FROM alpine:3.20
RUN apk add --no-cache ca-certificates
COPY --from=cloudflared /usr/local/bin/cloudflared /usr/local/bin/cloudflared
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]
