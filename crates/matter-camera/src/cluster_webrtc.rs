//! WebRtcTransportProvider cluster (0x0553) — Matter 1.5 §4.22
//!
//! Enables WebRTC-based streaming for Matter camera devices.

use std::sync::{Arc, RwLock};

use rs_matter::attributes;
use rs_matter::commands;
use rs_matter::dm::{
    Access, Attribute, Cluster, Command, Dataver, Handler, InvokeContext, InvokeReply,
    NonBlockingHandler, Quality, ReadContext, ReadReply, Reply,
};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::{TLVTag, TLVWrite};
use strum::FromRepr;
use tracing::debug;

use crate::types::CameraEndpointState;

pub const CLUSTER_ID: u32 = 0x0553;
const CLUSTER_REVISION: u16 = 1;

// ── Attribute enum ──

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Attributes {
    CurrentSessions = 0x0000,
}

rs_matter::attribute_enum!(Attributes);

// ── Command enum ──

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Commands {
    SolicitOffer = 0x0001,
    SolicitOfferResponse = 0x0002,
    ProvideOffer = 0x0003,
    ProvideOfferResponse = 0x0004,
    ProvideAnswer = 0x0005,
    ProvideIceCandidates = 0x0006,
    EndSession = 0x0007,
}

rs_matter::command_enum!(Commands);

// ── Cluster metadata ──

pub const WEBRTC_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    0, // feature_map
    attributes!(
        Attribute::new(Attributes::CurrentSessions as _, Access::RV, Quality::NONE)
    ),
    commands!(
        Command::new(Commands::ProvideOffer as _, Some(Commands::ProvideOfferResponse as _), Access::WO),
        Command::new(Commands::SolicitOffer as _, Some(Commands::SolicitOfferResponse as _), Access::WO),
        Command::new(Commands::ProvideAnswer as _, None, Access::WO),
        Command::new(Commands::ProvideIceCandidates as _, None, Access::WO),
        Command::new(Commands::EndSession as _, None, Access::WO)
    ),
    &[], // events: none
    |_, _, _| true, // with_attrs
    |_, _, _| true, // with_cmds
    |_, _, _| true, // with_events
);

// ── SDP exchange callback ──

/// Callback type for SDP offer/answer exchange.
/// Takes an SDP offer string, returns (sdp_answer, ice_candidates).
/// This is called synchronously from the Matter handler thread.
pub type SdpExchangeFn = Box<dyn Fn(&str) -> Result<(String, Vec<String>), String> + Send + Sync>;

// ── Handler ──

pub struct WebRtcHandler {
    dataver: Dataver,
    state: Arc<RwLock<CameraEndpointState>>,
    /// Optional SDP exchange callback — when None, returns a stub SDP answer.
    /// Set by the bridge after go2rtc is wired up.
    sdp_exchange: Option<SdpExchangeFn>,
}

impl WebRtcHandler {
    pub fn new(dataver: Dataver, state: Arc<RwLock<CameraEndpointState>>) -> Self {
        Self {
            dataver,
            state,
            sdp_exchange: None,
        }
    }

    /// Set the SDP exchange callback for real go2rtc integration.
    pub fn set_sdp_exchange(&mut self, f: SdpExchangeFn) {
        self.sdp_exchange = Some(f);
    }
}

impl Handler for WebRtcHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        if let Some(mut writer) = reply.with_dataver(self.dataver.get())? {
            if attr.is_system() {
                return WEBRTC_CLUSTER.read(attr, writer);
            }

            let state = self.state.read().map_err(|_| ErrorCode::Busy)?;

            match attr.attr_id.try_into()? {
                Attributes::CurrentSessions => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    for session in &state.current_sessions {
                        tw.start_struct(&TLVTag::Anonymous)?;
                        tw.u16(&TLVTag::Context(1), session.id)?;
                        tw.u64(&TLVTag::Context(2), session.peer_node_id)?;
                        tw.u8(&TLVTag::Context(3), session.peer_fabric_index)?;
                        tw.u8(&TLVTag::Context(4), session.stream_usage as u8)?;
                        if let Some(vid) = session.video_stream_id {
                            tw.u16(&TLVTag::Context(5), vid)?;
                        }
                        if let Some(aid) = session.audio_stream_id {
                            tw.u16(&TLVTag::Context(6), aid)?;
                        }
                        tw.end_container()?;
                    }
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
            }
        } else {
            Ok(())
        }
    }

    fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        let cmd = ctx.cmd();
        let data = ctx.data();
        let fields = data.structure()?;

        match cmd.cmd_id.try_into()? {
            Commands::ProvideOffer => {
                // Matter 1.5 ProvideOffer fields:
                //   ctx(0)  WebRTCSessionID — nullable u16 for session resumption (ignored here)
                //   ctx(1)  SDP — the offer string
                //   ctx(2)  ICEServers — optional list (not read; go2rtc/MediaMTX handles STUN)
                //   ctx(3)  ICETransportPolicy — optional enum8 (not read)
                //   ctx(4)  MetadataOptions — optional bitmap8 (not read)
                let sdp_offer = parse_provide_offer_sdp(&fields)?;
                let stream_usage = provide_offer_stream_usage(&fields);

                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                let session_id = state.next_session_id;
                state.next_session_id += 1;

                // Exchange SDP via go2rtc if available, otherwise return stub
                let sdp_answer = if let Some(ref exchange) = self.sdp_exchange {
                    match exchange(sdp_offer) {
                        Ok((answer, _candidates)) => {
                            debug!(
                                session_id,
                                sdp_answer_len = answer.len(),
                                "ProvideOffer — SDP exchanged via go2rtc"
                            );
                            answer
                        }
                        Err(e) => {
                            debug!(
                                session_id,
                                err = %e,
                                "ProvideOffer — go2rtc SDP exchange failed, returning stub"
                            );
                            "v=0\r\n".to_string()
                        }
                    }
                } else {
                    debug!(session_id, "ProvideOffer — no SDP exchanger, returning stub");
                    "v=0\r\n".to_string()
                };

                state.current_sessions.push(crate::types::WebRtcSession {
                    id: session_id,
                    peer_node_id: 0,
                    peer_fabric_index: cmd.fab_idx,
                    stream_usage,
                    video_stream_id: None,
                    audio_stream_id: None,
                    metadata_options: None,
                });
                self.dataver.changed();

                let mut writer = reply.with_command(Commands::ProvideOfferResponse as _)?;
                let tag = writer.tag().clone();
                let mut tw = writer.writer();
                tw.start_struct(&tag)?;
                tw.u16(&TLVTag::Context(0), session_id)?;
                tw.utf8(&TLVTag::Context(1), &sdp_answer)?;
                tw.end_container()?;
                drop(tw);
                writer.complete()
            }
            Commands::SolicitOffer => {
                debug!("SolicitOffer not implemented");
                Err(ErrorCode::CommandNotFound.into())
            }
            Commands::ProvideAnswer => {
                debug!("ProvideAnswer not implemented");
                Err(ErrorCode::CommandNotFound.into())
            }
            Commands::ProvideIceCandidates => {
                let session_id = fields.find_ctx(0)?.u16().unwrap_or(0);
                debug!(session_id, "ProvideIceCandidates (stub — ignoring)");
                Ok(())
            }
            Commands::EndSession => {
                let session_id = fields.find_ctx(0)?.u16().unwrap_or(0);
                let reason = fields.find_ctx(1)?.u8().unwrap_or(11);

                let mut state = self.state.write().map_err(|_| ErrorCode::Busy)?;
                state.current_sessions.retain(|s| s.id != session_id);
                self.dataver.changed();

                debug!(session_id, reason, "EndSession");
                Ok(())
            }
            Commands::SolicitOfferResponse | Commands::ProvideOfferResponse => {
                Err(ErrorCode::CommandNotFound.into())
            }
        }
    }
}

impl NonBlockingHandler for WebRtcHandler {}

fn parse_provide_offer_sdp<'a>(fields: &rs_matter::tlv::TLVSequence<'a>) -> Result<&'a str, Error> {
    let sdp = fields
        .find_ctx(1)?
        .utf8()
        .map_err(|_| ErrorCode::InvalidCommand)?;

    if sdp.trim().is_empty() {
        return Err(ErrorCode::InvalidCommand.into());
    }

    Ok(sdp)
}

fn provide_offer_stream_usage(
    _fields: &rs_matter::tlv::TLVSequence<'_>,
) -> crate::types::StreamUsage {
    // StreamUsage is not a ProvideOffer field in Matter 1.5. Always default to LiveView.
    crate::types::StreamUsage::LiveView
}

#[cfg(test)]
mod tests {
    use rs_matter::tlv::{TLVElement, TLVTag, TLVWrite};
    use rs_matter::utils::storage::WriteBuf;

    use super::{parse_provide_offer_sdp, provide_offer_stream_usage};
    use crate::types::StreamUsage;

    fn build_provide_offer_fields(
        sdp_ctx1: Option<&str>,
        ctx2_u8: Option<u8>,
        ctx3_u8: Option<u8>,
    ) -> Vec<u8> {
        let mut backing = vec![0_u8; 256];
        let mut tw = WriteBuf::new(backing.as_mut_slice());
        tw.start_struct(&TLVTag::Anonymous).unwrap();

        if let Some(sdp) = sdp_ctx1 {
            tw.utf8(&TLVTag::Context(1), sdp).unwrap();
        }
        if let Some(v) = ctx2_u8 {
            tw.u8(&TLVTag::Context(2), v).unwrap();
        }
        if let Some(v) = ctx3_u8 {
            tw.u8(&TLVTag::Context(3), v).unwrap();
        }

        tw.end_container().unwrap();

        tw.as_slice().to_vec()
    }

    #[test]
    fn provide_offer_reads_sdp_from_ctx1() {
        let encoded =
            build_provide_offer_fields(Some("v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n"), None, None);
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();

        let sdp = parse_provide_offer_sdp(&fields).unwrap();
        assert_eq!(sdp, "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n");
    }

    #[test]
    fn provide_offer_allows_optional_fields_to_be_omitted() {
        let encoded = build_provide_offer_fields(Some("v=0\r\n"), None, None);
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();

        let sdp = parse_provide_offer_sdp(&fields).unwrap();
        assert_eq!(sdp, "v=0\r\n");
    }

    #[test]
    fn provide_offer_ctx2_does_not_change_stream_usage() {
        let encoded = build_provide_offer_fields(Some("v=0\r\n"), Some(99), Some(7));
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();

        assert_eq!(provide_offer_stream_usage(&fields), StreamUsage::LiveView);
    }

    #[test]
    fn provide_offer_missing_sdp_fails() {
        let encoded = build_provide_offer_fields(None, None, None);
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();

        assert!(parse_provide_offer_sdp(&fields).is_err());
    }

    #[test]
    fn provide_offer_malformed_sdp_field_fails() {
        let mut backing = vec![0_u8; 256];
        let mut tw = WriteBuf::new(backing.as_mut_slice());
        tw.start_struct(&TLVTag::Anonymous).unwrap();
        tw.u8(&TLVTag::Context(1), 42).unwrap();
        tw.end_container().unwrap();

        let encoded = tw.as_slice().to_vec();
        let root = TLVElement::new(&encoded);
        let fields = root.structure().unwrap();

        assert!(parse_provide_offer_sdp(&fields).is_err());
    }
}
