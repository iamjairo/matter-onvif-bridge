use oxvif::OnvifClient;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let url = format!("http://{}:{}/onvif/device_service", args[1], args[2]);
    let c = OnvifClient::new(&url).with_credentials(&args[3], &args[4]);
    let dt = c.get_system_date_and_time().await?;
    let c = c.with_utc_offset(dt.utc_offset_secs());
    for s in c.get_scopes().await? { println!("{}", s); }
    Ok(())
}
