use oxvif::OnvifClient;
use std::time::Duration;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let user = args.get(1).cloned().unwrap_or_else(|| "admin".into());
    let pass = args.get(2).cloned().unwrap_or_else(|| "PassMeIn815@".into());
    let devices = oxvif::discovery::probe(Duration::from_secs(3)).await;
    println!("WS-Discovery returned {} devices", devices.len());
    for (i, d) in devices.iter().enumerate() {
        println!("\n[{}] xaddrs={:?} types={:?}", i, d.xaddrs, d.types);
        for xaddr in &d.xaddrs {
            let c = OnvifClient::new(xaddr).with_credentials(&user, &pass);
            match c.get_system_date_and_time().await {
                Ok(dt) => {
                    let c = c.with_utc_offset(dt.utc_offset_secs());
                    match c.get_device_info().await {
                        Ok(info) => println!("  {} → {} {} sn={}", xaddr, info.manufacturer, info.model, info.serial_number),
                        Err(e) => println!("  {} → GetDeviceInfo failed: {e}", xaddr),
                    }
                }
                Err(e) => println!("  {} → time sync failed: {e}", xaddr),
            }
        }
    }
    Ok(())
}
