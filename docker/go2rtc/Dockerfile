FROM alexxit/go2rtc:1.9.7

# Do not bake go2rtc.yaml into /config inside the image: go2rtc persists
# dynamically added streams to that path, which can write credential-bearing
# RTSP URLs into the container filesystem. Provide /config/go2rtc.yaml as a
# read-only runtime mount instead so dynamic registrations remain memory-only.

# The official image already exposes ports 1984 (API), 8555 (WebRTC), 8554 (RTSP)
