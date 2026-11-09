# syntax=docker/dockerfile:1
FROM golang:latest AS age-build
RUN CGO_ENABLED=0 go install filippo.io/age/cmd/age@latest && \
    CGO_ENABLED=0 go install filippo.io/age/cmd/age-keygen@latest

FROM postgres:18-trixie AS backup
RUN apt-get update && apt-get install -y --no-install-recommends python3 && rm -rf /var/lib/apt/lists/* && \
    useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin liar && \
    mkdir -p /app && chmod 0755 /app
COPY --from=age-build /go/bin/age /go/bin/age-keygen /usr/local/bin/
COPY --chmod=0444 deploy/backup.py /app/backup.py
USER 10001:10001
STOPSIGNAL SIGTERM
ENTRYPOINT ["python3", "/app/backup.py"]
CMD ["backup"]
