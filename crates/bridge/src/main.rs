#![recursion_limit = "2048"]

mod config;
mod mdns;
mod onvif_bridge;
mod slot_persistence;

use core::pin::pin;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::RwLock;

use embassy_futures::select::select4;
use rand::rand_core::RngCore;

use rs_matter::crypto::{Crypto, default_crypto};
use rs_matter::dm::DeviceType;
use rs_matter::dm::clusters::desc::{self, ClusterHandler as _};
use rs_matter::dm::clusters::groups::{self, ClusterHandler as _};
use rs_matter::dm::clusters::net_comm::SharedNetworks;
use rs_matter::dm::devices::test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_DEV_COMM};
use rs_matter::dm::devices::{DEV_TYPE_AGGREGATOR, DEV_TYPE_BRIDGED_NODE};
use rs_matter::dm::endpoints;
use rs_matter::dm::events::Events;
use rs_matter::dm::networks::SysNetifs;
use rs_matter::dm::networks::eth::EthNetwork;
use rs_matter::dm::subscriptions::Subscriptions;
use rs_matter::dm::{
    Async, AsyncHandler, AsyncMetadata, DataModel, Dataver, EmptyHandler, Endpoint, EpClMatcher,
    Node,
};
use rs_matter::error::Error;
use rs_matter::pairing::DiscoveryCapabilities;
use rs_matter::pairing::qr::QrTextType;
use rs_matter::persist::{DirKvBlobStore, SharedKvBlobStore};
use rs_matter::respond::DefaultResponder;
use rs_matter::sc::pase::MAX_COMM_WINDOW_TIMEOUT_SECS;
use rs_matter::transport::MATTER_SOCKET_BIND_ADDR;
use rs_matter::utils::select::Coalesce;
use rs_matter::utils::storage::pooled::PooledBuffers;
use rs_matter::{Matter, clusters, devices, root_endpoint};

use matter_camera::cluster_av_stream::{AV_STREAM_CLUSTER, AvStreamHandler, SnapshotCaptureResult};
use matter_camera::cluster_occupancy::{OCCUPANCY_CLUSTER, OccupancyDataver, OccupancyHandler};
use matter_camera::cluster_webrtc::{WEBRTC_CLUSTER, WebRtcHandler};
use matter_camera::types::CameraEndpointState;

pub use rs_matter::dm::clusters::decl::bridged_device_basic_information::{
    self, ClusterHandler as _, KeepActiveRequest,
};
use rs_matter::dm::{InvokeContext, ReadContext};
use rs_matter::tlv::{TLVBuilderParent, Utf8StrBuilder};
use rs_matter::with;

/// Camera device type — Matter 1.5 Basic Video Camera (0x0103)
const DEV_TYPE_CAMERA: DeviceType = DeviceType {
    dtype: 0x0103,
    drev: 1,
};

/// Maximum number of camera endpoints we pre-allocate.
///
/// Google Home walks every endpoint in the descriptor and shows even
/// `reachable=false` ones as "unavailable" devices, so the slot pool needs
/// to be sized close to the actual number of cameras. 8 leaves a small
/// headroom over the 6 cameras the user currently runs; bumping requires
/// a recompile because the static `Node` array and the handler chain are
/// both type-level constructs.
pub const MAX_CAMERAS: usize = 8;

/// How many of the pre-allocated camera slots include the OccupancySensing
/// cluster (for cameras whose ONVIF device advertises a MotionAlarm topic).
/// The remaining `MAX_CAMERAS - WITH_OCCUPANCY_CAMERAS` slots are camera-only.
pub const WITH_OCCUPANCY_CAMERAS: usize = 7;

/// Endpoint IDs: 0 = root, 1 = aggregator, 2..8 = camera+occupancy slots,
/// 9 = camera-only slot.
const AGGREGATOR_EP: u16 = 1;
#[allow(dead_code)]
const CAMERA_EP_START: u16 = 2;

fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();

    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let cfg = config::Config::from_env();
    log::info!(
        "Matter-ONVIF Bridge (Rust) — port={}, passcode={}, discriminator={}",
        cfg.matter.port,
        cfg.matter.passcode,
        cfg.matter.discriminator
    );

    // Root device basic info. This describes the bridge itself, not the
    // bridged cameras — each camera's serial/hw/sw is exposed via its own
    // BridgedDeviceBasicInformation cluster (see BridgedHandler below).
    //
    // The serial number is derived from the bridge host's hostname so it's
    // stable per-deployment. Software version is taken from the crate's
    // Cargo.toml at compile time.
    let host_serial: &'static str = Box::leak(
        hostname::get()
            .ok()
            .and_then(|s| s.into_string().ok())
            .map(|h| format!("matter-onvif-bridge@{h}"))
            .unwrap_or_else(|| "matter-onvif-bridge".to_string())
            .into_boxed_str(),
    );
    let dev_det = rs_matter::dm::clusters::basic_info::BasicInfoConfig {
        vid: 0xFFF1, // Test vendor ID (required for test DAC attestation)
        pid: 0x8001, // Test product ID
        product_name: "Matter-ONVIF Camera Bridge",
        vendor_name: "matter-onvif-bridge",
        device_name: "Camera Bridge",
        serial_no: host_serial,
        hw_ver: 1,
        hw_ver_str: "1.0",
        sw_ver: 1,
        sw_ver_str: env!("CARGO_PKG_VERSION"),
        ..rs_matter::dm::clusters::basic_info::BasicInfoConfig::new()
    };
    let mut matter = Matter::new_default(&dev_det, TEST_DEV_COMM, &TEST_DEV_ATT, cfg.matter.port);

    let buffers = PooledBuffers::<10, _>::new(0);
    let subscriptions: Subscriptions = Subscriptions::new();
    let crypto = default_crypto(rand::rng(), DAC_PRIVKEY);
    let mut rand = crypto.rand()?;
    let mut events: Events = Events::new_default();

    // Persistence — the new rs-matter uses a KvBlobStore model instead of the
    // old Psm. Load persisted state before building the data model.
    let persist_path = std::path::PathBuf::from(&cfg.matter.storage_path);
    let mut kv = DirKvBlobStore::new(persist_path);
    let mut kv_buf = [0u8; 4096];
    futures_lite::future::block_on(matter.load_persist(&mut kv, &mut kv_buf))?;
    futures_lite::future::block_on(events.load_persist(&mut kv, &mut kv_buf))?;

    // Create shared state for each camera endpoint
    let camera_states: Vec<Arc<RwLock<CameraEndpointState>>> = (0..MAX_CAMERAS)
        .map(|_| Arc::new(RwLock::new(CameraEndpointState::default())))
        .collect();

    // Create handlers for camera endpoints
    let mut av_handlers: Vec<AvStreamHandler> = camera_states
        .iter()
        .map(|s| AvStreamHandler::new(Dataver::new_rand(&mut rand), Arc::clone(s)))
        .collect();
    let mut webrtc_handlers: Vec<WebRtcHandler> = camera_states
        .iter()
        .map(|s| WebRtcHandler::new(Dataver::new_rand(&mut rand), Arc::clone(s)))
        .collect();

    // OccupancySensing handlers — only for the first WITH_OCCUPANCY_CAMERAS slots.
    // Each handler holds an Arc<Dataver> so the bridge thread can bump the
    // dataver from outside the Matter executor when a new ONVIF event arrives.
    let occupancy_datavers: Vec<OccupancyDataver> = (0..WITH_OCCUPANCY_CAMERAS)
        .map(|_| OccupancyDataver::new(rand.next_u32()))
        .collect();
    let occupancy_handlers: Vec<OccupancyHandler> = (0..WITH_OCCUPANCY_CAMERAS)
        .map(|i| {
            OccupancyHandler::new(occupancy_datavers[i].clone(), Arc::clone(&camera_states[i]))
        })
        .collect();

    // Start ONVIF + go2rtc bridge on a separate tokio thread
    let registry = onvif_client::registry::CameraRegistry::new(64);
    let media_bridge =
        onvif_bridge::start_onvif_bridge(&cfg, &camera_states, &occupancy_datavers, registry);

    // Wire SDP exchange callbacks into WebRTC handlers BEFORE building the data model.
    // Each handler gets a closure that looks up its stream name and calls the media server.
    for (i, handler) in webrtc_handlers.iter_mut().enumerate() {
        let api = media_bridge.api.clone();
        let stream_names = media_bridge.stream_names.clone();

        handler.set_sdp_exchange(Box::new(move |sdp_offer: &str| {
            let stream_name = {
                let names = stream_names
                    .read()
                    .map_err(|e| format!("Lock error: {e}"))?;
                names
                    .get(&i)
                    .cloned()
                    .ok_or_else(|| format!("No stream registered for endpoint slot {i}"))?
            };

            // Run the async SDP exchange on a one-shot tokio runtime
            // (we're on the Matter executor thread, not a tokio context)
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

            let api = api.clone();
            rt.block_on(async {
                let session = media::webrtc_session::WebRtcSession::new(stream_name, api);
                let result = session.negotiate(sdp_offer).await?;
                Ok((result.sdp_answer, result.ice_candidates))
            })
        }));
    }

    // Wire snapshot capture callbacks into AV handlers.
    for (i, handler) in av_handlers.iter_mut().enumerate() {
        let api = media_bridge.api.clone();
        let stream_names = media_bridge.stream_names.clone();

        handler.set_snapshot_capture(Box::new(move |request| {
            let stream_name = {
                let names = stream_names
                    .read()
                    .map_err(|e| format!("Lock error: {e}"))?;
                names
                    .get(&i)
                    .cloned()
                    .ok_or_else(|| format!("No stream registered for endpoint slot {i}"))?
            };

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

            let api = api.clone();
            rt.block_on(async move {
                let jpeg_bytes = api.snapshot_jpeg(&stream_name).await?;
                let resolution =
                    jpeg_dimensions(&jpeg_bytes).unwrap_or(request.requested_resolution);

                Ok(SnapshotCaptureResult {
                    jpeg_bytes,
                    resolution,
                })
            })
        }));
    }

    // Build the data model handler (borrows handlers immutably from here on)
    let dm = DataModel::new(
        &matter,
        &crypto,
        &buffers,
        &subscriptions,
        &events,
        dm_handler(
            rand,
            &av_handlers,
            &webrtc_handlers,
            &occupancy_handlers,
            &camera_states,
        ),
        SharedKvBlobStore::new(kv, kv_buf.as_mut_slice()),
        SharedNetworks::new(EthNetwork::new_default()),
    );

    let responder = DefaultResponder::new(&dm);
    let mut respond = pin!(responder.run::<4, 4>());
    let mut dm_job = pin!(dm.run());

    // Create a dual-stack UDP socket explicitly via socket2.
    // Rust's std UdpSocket::bind on "[::]:port" may set IPV6_V6ONLY=1,
    // which prevents receiving IPv6 packets from chip-tool.
    let socket = {
        let s = socket2::Socket::new(
            socket2::Domain::IPV6,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )?;
        s.set_only_v6(false)?;
        s.set_reuse_address(true)?;
        s.bind(&MATTER_SOCKET_BIND_ADDR.into())?;
        s.set_nonblocking(true)?;
        async_io::Async::<UdpSocket>::new_nonblocking(s.into())?
    };

    let mut mdns = pin!(mdns::run_mdns(&matter, &crypto));
    let mut transport = pin!(matter.run(&crypto, &socket, &socket, &socket));

    if !matter.is_commissioned() {
        log::info!("Device not commissioned. Displaying QR code...");
        matter.print_standard_qr_text(DiscoveryCapabilities::IP)?;
        matter.print_standard_qr_code(QrTextType::Unicode, DiscoveryCapabilities::IP)?;
        matter.open_basic_comm_window(MAX_COMM_WINDOW_TIMEOUT_SECS, &crypto, dm.change_notify())?;
    } else {
        log::info!("Device already commissioned.");
    }

    let all = select4(&mut transport, &mut mdns, &mut respond, &mut dm_job);

    futures_lite::future::block_on(all.coalesce())
}

fn jpeg_dimensions(data: &[u8]) -> Option<matter_camera::types::VideoResolution> {
    if data.len() < 4 || data.first() != Some(&0xFF) || data.get(1) != Some(&0xD8) {
        return None;
    }

    let mut i = 2usize;
    while i + 8 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }

        let marker = data[i + 1];
        i += 2;

        // Standalone markers without segment length.
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }

        if i + 2 > data.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            return None;
        }

        // Baseline/progressive SOF markers.
        if matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        ) && i + 7 < data.len()
        {
            let height = u16::from_be_bytes([data[i + 3], data[i + 4]]);
            let width = u16::from_be_bytes([data[i + 5], data[i + 6]]);
            if width > 0 && height > 0 {
                return Some(matter_camera::types::VideoResolution { width, height });
            }
            return None;
        }

        i += seg_len;
    }

    None
}

// ── Node definition ──

/// Build the static Node metadata with pre-allocated camera endpoints.
///
/// We use a macro to generate the endpoint array at compile time since
/// `Node` requires `&'static [Endpoint]`.
macro_rules! camera_endpoints {
    (
        with_occupancy: [$($occ_id:expr),* $(,)?],
        plain: [$($plain_id:expr),* $(,)?] $(,)?
    ) => {
        &[
            // Endpoint 0: Root
            root_endpoint!(geth),
            // Endpoint 1: Aggregator
            Endpoint {
                id: AGGREGATOR_EP,
                device_types: devices!(DEV_TYPE_AGGREGATOR),
                clusters: clusters!(desc::DescHandler::CLUSTER),
            },
            // Camera endpoints with OccupancySensing
            $(
                Endpoint {
                    id: $occ_id,
                    device_types: devices!(DEV_TYPE_CAMERA, DEV_TYPE_BRIDGED_NODE),
                    clusters: clusters!(
                        desc::DescHandler::CLUSTER,
                        groups::GroupsHandler::CLUSTER,
                        BridgedHandler::CLUSTER,
                        AV_STREAM_CLUSTER,
                        WEBRTC_CLUSTER,
                        OCCUPANCY_CLUSTER
                    ),
                },
            )*
            // Camera-only endpoints (no OccupancySensing)
            $(
                Endpoint {
                    id: $plain_id,
                    device_types: devices!(DEV_TYPE_CAMERA, DEV_TYPE_BRIDGED_NODE),
                    clusters: clusters!(
                        desc::DescHandler::CLUSTER,
                        groups::GroupsHandler::CLUSTER,
                        BridgedHandler::CLUSTER,
                        AV_STREAM_CLUSTER,
                        WEBRTC_CLUSTER
                    ),
                },
            )*
        ]
    }
}

const NODE: Node<'static> = Node {
    endpoints: camera_endpoints![
        with_occupancy: [2, 3, 4, 5, 6, 7, 8],
        plain: [9],
    ],
};

// ── BridgedDeviceBasicInformation handler ──

#[derive(Clone)]
pub struct BridgedHandler {
    dataver: Dataver,
    state: Arc<RwLock<CameraEndpointState>>,
}

impl BridgedHandler {
    pub fn new(dataver: Dataver, state: Arc<RwLock<CameraEndpointState>>) -> Self {
        Self { dataver, state }
    }

    pub fn adapt(self) -> bridged_device_basic_information::HandlerAdaptor<Self> {
        bridged_device_basic_information::HandlerAdaptor(self)
    }

    fn read_state_string(&self, f: impl Fn(&CameraEndpointState) -> &str) -> String {
        self.state
            .read()
            .map(|s| f(&s).to_string())
            .unwrap_or_default()
    }
}

impl bridged_device_basic_information::ClusterHandler for BridgedHandler {
    const CLUSTER: rs_matter::dm::Cluster<'static> = bridged_device_basic_information::FULL_CLUSTER
        .with_features(0)
        .with_attrs(with!(all))
        .with_cmds(with!());

    fn dataver(&self) -> u32 {
        self.dataver.get()
    }

    fn dataver_changed(&self) {
        self.dataver.changed();
    }

    fn reachable(&self, _ctx: impl ReadContext) -> Result<bool, Error> {
        Ok(self.state.read().map(|s| s.occupied).unwrap_or(false))
    }

    fn unique_id<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.unique_id);
        builder.set(&val)
    }

    fn vendor_name<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.vendor_name);
        builder.set(&val)
    }

    fn product_name<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.product_name);
        builder.set(&val)
    }

    fn node_label<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.node_label);
        builder.set(&val)
    }

    fn serial_number<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.serial_number);
        builder.set(&val)
    }

    fn hardware_version_string<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.hardware_version_string);
        builder.set(&val)
    }

    fn software_version_string<P: TLVBuilderParent>(
        &self,
        _ctx: impl ReadContext,
        builder: Utf8StrBuilder<P>,
    ) -> Result<P, Error> {
        let val = self.read_state_string(|s| &s.software_version_string);
        builder.set(&val)
    }

    fn handle_keep_active(
        &self,
        _ctx: impl InvokeContext,
        _request: KeepActiveRequest<'_>,
    ) -> Result<(), Error> {
        Err(rs_matter::error::ErrorCode::CommandNotFound.into())
    }
}

// ── Data Model handler composition ──

/// Compose the full data model handler chain.
///
/// This chains the root endpoint system handlers with the aggregator descriptor
/// and all pre-allocated camera endpoint handlers.
fn dm_handler<'a>(
    mut rand: impl RngCore + Copy,
    av_handlers: &'a [AvStreamHandler],
    webrtc_handlers: &'a [WebRtcHandler],
    occupancy_handlers: &'a [OccupancyHandler],
    camera_states: &'a [Arc<RwLock<CameraEndpointState>>],
) -> impl AsyncMetadata + AsyncHandler + 'a {
    // Build the camera endpoint handler chain
    let chain = EmptyHandler
        // Aggregator endpoint descriptor
        .chain(
            EpClMatcher::new(Some(AGGREGATOR_EP), Some(desc::DescHandler::CLUSTER.id)),
            Async(desc::DescHandler::new_aggregator(Dataver::new_rand(&mut rand)).adapt()),
        );

    // For each camera endpoint, chain in all cluster handlers
    // We do this inline with a macro since the chain type must be known at compile time.
    // However, since we're returning `impl AsyncHandler`, we can build it dynamically
    // using a loop — but rs-matter's ChainedHandler is generic, so each .chain() call
    // changes the type. We need to use a macro or manual unrolling.
    //
    // For Phase 1, we'll use a helper that chains all 16 endpoints.
    // Common cluster handlers shared by every camera endpoint variant.
    macro_rules! chain_camera_base {
        ($chain:expr, $rand:expr, $av:expr, $webrtc:expr, $states:expr, $ep:expr, $idx:expr) => {
            $chain
                .chain(
                    EpClMatcher::new(Some($ep), Some(desc::DescHandler::CLUSTER.id)),
                    Async(desc::DescHandler::new(Dataver::new_rand(&mut $rand)).adapt()),
                )
                .chain(
                    EpClMatcher::new(Some($ep), Some(groups::GroupsHandler::CLUSTER.id)),
                    Async(groups::GroupsHandler::new(Dataver::new_rand(&mut $rand)).adapt()),
                )
                .chain(
                    EpClMatcher::new(Some($ep), Some(BridgedHandler::CLUSTER.id)),
                    Async(
                        BridgedHandler::new(
                            Dataver::new_rand(&mut $rand),
                            Arc::clone(&$states[$idx]),
                        )
                        .adapt(),
                    ),
                )
                .chain(
                    EpClMatcher::new(Some($ep), Some(AV_STREAM_CLUSTER.id)),
                    Async(&$av[$idx]),
                )
                .chain(
                    EpClMatcher::new(Some($ep), Some(WEBRTC_CLUSTER.id)),
                    Async(&$webrtc[$idx]),
                )
        };
    }

    // Endpoints 2..=8: camera + OccupancySensing (7 slots).
    macro_rules! chain_camera_ep_with_occupancy {
        ($chain:expr, $rand:expr, $av:expr, $webrtc:expr, $occ:expr, $states:expr, $ep:expr, $idx:expr) => {
            chain_camera_base!($chain, $rand, $av, $webrtc, $states, $ep, $idx).chain(
                EpClMatcher::new(Some($ep), Some(OCCUPANCY_CLUSTER.id)),
                Async(&$occ[$idx]),
            )
        };
    }

    // Endpoints 2..=8: camera + OccupancySensing (7 slots).
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        2,
        0
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        3,
        1
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        4,
        2
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        5,
        3
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        6,
        4
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        7,
        5
    );
    let chain = chain_camera_ep_with_occupancy!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        occupancy_handlers,
        camera_states,
        8,
        6
    );
    // Endpoint 9: camera-only (1 slot).
    let chain = chain_camera_base!(
        chain,
        rand,
        av_handlers,
        webrtc_handlers,
        camera_states,
        9,
        7
    );

    (
        NODE,
        endpoints::with_eth_sys(&false, &(), &SysNetifs, rand, chain),
    )
}
