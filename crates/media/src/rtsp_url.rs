use url::Url;

/// Redact RTSP credentials for safe logging while preserving endpoint context.
pub(crate) fn redact_rtsp_url_for_log(rtsp_url: &str) -> String {
    if rtsp_url.is_empty() {
        return String::new();
    }

    if let Ok(mut parsed) = Url::parse(rtsp_url) {
        let has_username = !parsed.username().is_empty();
        let has_password = parsed.password().is_some();

        if has_username {
            let _ = parsed.set_username("***");
        }
        if has_password {
            let _ = parsed.set_password(Some("***"));
        }

        return parsed.to_string();
    }

    redact_userinfo_fallback(rtsp_url)
}

fn redact_userinfo_fallback(rtsp_url: &str) -> String {
    let Some(scheme_sep) = rtsp_url.find("://") else {
        return rtsp_url.to_string();
    };
    let auth_start = scheme_sep + 3;
    let Some(at_rel) = rtsp_url[auth_start..].find('@') else {
        return rtsp_url.to_string();
    };
    let at_pos = auth_start + at_rel;
    let userinfo = &rtsp_url[auth_start..at_pos];

    let replacement = if userinfo.contains(':') {
        "***:***"
    } else {
        "***"
    };

    format!(
        "{}{}{}",
        &rtsp_url[..auth_start],
        replacement,
        &rtsp_url[at_pos..]
    )
}

#[cfg(test)]
mod tests {
    use super::redact_rtsp_url_for_log;

    #[test]
    fn redacts_username_and_password_when_present() {
        assert_eq!(
            redact_rtsp_url_for_log("rtsp://user:secret@192.168.1.10:554/stream"),
            "rtsp://***:***@192.168.1.10:554/stream"
        );
    }

    #[test]
    fn redacts_username_when_password_not_present() {
        assert_eq!(
            redact_rtsp_url_for_log("rtsp://user@192.168.1.10:554/stream"),
            "rtsp://***@192.168.1.10:554/stream"
        );
    }

    #[test]
    fn keeps_url_without_credentials_unchanged() {
        assert_eq!(
            redact_rtsp_url_for_log("rtsp://192.168.1.10:554/stream"),
            "rtsp://192.168.1.10:554/stream"
        );
    }

    #[test]
    fn redacts_credentials_in_edge_case_with_extra_at() {
        assert_eq!(
            redact_rtsp_url_for_log("rtsp://user:secret@@example/stream"),
            "rtsp://***:***@example/stream"
        );
    }
}
