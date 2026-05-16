FROM alexxit/go2rtc:1.9.7

COPY go2rtc.yaml /config/go2rtc.yaml

# Keep a minimal checked-in config in the image so `docker compose up` works
# from a clean checkout. Streams are still registered dynamically by the bridge
# via the go2rtc REST API.
