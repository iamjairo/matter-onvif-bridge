//! Probe a single ONVIF camera for event topics + motion notifications.
//!
//! Usage:
//!   cargo run -p onvif-client --example probe_motion -- <host> <port> <user> <pass>
//!
//! Lists all advertised event topics, then subscribes WITHOUT a filter and
//! prints every notification (topic + data) for 30 seconds.

use std::env;
use std::time::Duration;

use oxvif::OnvifClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: probe_motion <host> <port> <user> <pass>");
        std::process::exit(2);
    }
    let host = &args[1];
    let port: u16 = args[2].parse()?;
    let user = &args[3];
    let pass = &args[4];

    let device_url = format!("http://{}:{}/onvif/device_service", host, port);
    let client = OnvifClient::new(&device_url).with_credentials(user, pass);

    let dt = client.get_system_date_and_time().await?;
    let client = client.with_utc_offset(dt.utc_offset_secs());

    let caps = client.get_capabilities().await?;
    let events_url = caps.events.url.as_deref().ok_or("no events url")?;
    println!("events_url = {}", events_url);

    let props = client.get_event_properties(events_url).await?;
    println!("\n== topics ({}):", props.topics.len());
    for t in &props.topics {
        println!("  {}", t);
    }

    println!("\n== subscribing (no filter) ==");
    let sub = client
        .create_pull_point_subscription(events_url, None, Some("PT60S"))
        .await?;
    println!("reference = {}", sub.reference_url);

    println!("\n== pulling for 30s ==");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        match client.pull_messages(&sub.reference_url, "PT5S", 32).await {
            Ok(msgs) => {
                if msgs.is_empty() {
                    println!("(empty pull)");
                } else {
                    for m in msgs {
                        println!(
                            "topic={} time={} source={:?} data={:?}",
                            m.topic, m.utc_time, m.source, m.data
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("PullMessages error: {e}");
                break;
            }
        }
    }

    let _ = client.unsubscribe(&sub.reference_url).await;
    Ok(())
}
