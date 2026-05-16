/// Redact credential-bearing userinfo in RTSP URLs before logging.
///
/// Keeps scheme/host/port/path/query/fragment for debugging while masking
/// username/password in the authority segment.
pub(crate) fn redact_rtsp_url_for_logs(rtsp_url: &str) -> String {
    if rtsp_url.is_empty() {
        return String::new();
    }

    match url::Url::parse(rtsp_url) {
        Ok(mut parsed) => {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("******");
                let _ = parsed.set_password(Some("******"));
            }
            parsed.to_string()
        }
        Err(_) => rtsp_url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::redact_rtsp_url_for_logs;

    #[test]
    fn redacts_username_and_password() {
        assert_eq!(
            redact_rtsp_url_for_logs("rtsp://admin:pass@192.168.1.10:554/stream"),
            "rtsp://******:******@192.168.1.10:554/stream",
        );
    }

    #[test]
    fn redacts_username_only() {
        assert_eq!(
            redact_rtsp_url_for_logs("rtsp://admin@camera.local:8554/live"),
            "rtsp://******:******@camera.local:8554/live",
        );
    }

    #[test]
    fn leaves_url_without_credentials_unchanged() {
        assert_eq!(
            redact_rtsp_url_for_logs("rtsp://192.168.1.10:554/stream"),
            "rtsp://192.168.1.10:554/stream",
        );
    }

    #[test]
    fn leaves_unparsable_input_unchanged() {
        assert_eq!(
            redact_rtsp_url_for_logs("not-a-valid-url"),
            "not-a-valid-url"
        );
    }
}
