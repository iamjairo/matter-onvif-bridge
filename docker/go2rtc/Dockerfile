FROM alexxit/go2rtc:1.9.7

# Bake the minimal config into the image so a clean checkout works without
# any host-side bind mounts.
COPY go2rtc.yaml /config/go2rtc.yaml

# The official image already exposes ports 1984 (API), 8555 (WebRTC), 8554 (RTSP)
