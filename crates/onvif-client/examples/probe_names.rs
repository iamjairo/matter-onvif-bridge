use oxvif::OnvifClient;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let url = format!("http://{}:{}/onvif/device_service", args[1], args[2]);
    let c = OnvifClient::new(&url).with_credentials(&args[3], &args[4]);
    let dt = c.get_system_date_and_time().await?;
    let c = c.with_utc_offset(dt.utc_offset_secs());
    println!("hostname: {:?}", c.get_hostname().await);
    let caps = c.get_capabilities().await?;
    if let Some(media_url) = caps.media.url.as_deref() {
        let profs = c.get_profiles(media_url).await?;
        for p in &profs {
            println!("profile token={} name={}", p.token, p.name);
        }
    }
    Ok(())
}
