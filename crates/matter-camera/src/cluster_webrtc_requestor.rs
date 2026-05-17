//! WebRtcTransportRequestor cluster (0x0554) — Matter 1.5 §9.10
//!
//! Client-initiated counterpart to WebRtcTransportProvider (0x0553).
//! Enables a Matter controller to initiate WebRTC sessions by sending
//! an SDP offer and receiving an answer from the camera device.

use rs_matter::attributes;
use rs_matter::commands;
use rs_matter::dm::{
    Access, Attribute, Cluster, Command, Dataver, Handler, InvokeContext, InvokeReply,
    NonBlockingHandler, Quality, ReadContext, ReadReply, Reply, WriteContext,
};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::{TLVTag, TLVWrite};
use strum::FromRepr;
use tracing::debug;

pub const CLUSTER_ID: u32 = 0x0554;
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
    Offer = 0x01,
    OfferResponse = 0x02,
    Answer = 0x03,
    ICECandidate = 0x04,
    End = 0x05,
}

rs_matter::command_enum!(Commands);

// ── Cluster metadata ──

pub const WEBRTC_REQUESTOR_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    0, // feature_map
    attributes!(Attribute::new(
        Attributes::CurrentSessions as _,
        Access::RV,
        Quality::NONE
    )),
    commands!(
        Command::new(
            Commands::Offer as _,
            Some(Commands::OfferResponse as _),
            Access::WO
        ),
        Command::new(Commands::Answer as _, None, Access::WO),
        Command::new(Commands::ICECandidate as _, None, Access::WO),
        Command::new(Commands::End as _, None, Access::WO)
    ),
    &[],            // events: none
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

pub struct WebRtcRequestorHandler {
    dataver: Dataver,
    /// Optional SDP exchange callback — when None, returns an error on Offer.
    /// Set by the bridge after the configured media backend is wired up.
    sdp_exchange: Option<SdpExchangeFn>,
}

impl WebRtcRequestorHandler {
    pub fn new(dataver: Dataver) -> Self {
        Self {
            dataver,
            sdp_exchange: None,
        }
    }

    /// Set the SDP exchange callback for real media-backend integration.
    pub fn set_sdp_exchange(&mut self, f: SdpExchangeFn) {
        self.sdp_exchange = Some(f);
    }
}

impl Handler for WebRtcRequestorHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        if let Some(mut writer) = reply.with_dataver(self.dataver.get())? {
            if attr.is_system() {
                return WEBRTC_REQUESTOR_CLUSTER.read(attr, writer);
            }

            match attr.attr_id.try_into()? {
                Attributes::CurrentSessions => {
                    // Return empty list — no active sessions tracked yet.
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
            }
        } else {
            Ok(())
        }
    }

    fn write(&self, _ctx: impl WriteContext) -> Result<(), Error> {
        Err(ErrorCode::AttributeNotFound.into())
    }

    fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        let cmd = ctx.cmd();
        let data = ctx.data();
        let fields = data.structure()?;

        match cmd.cmd_id.try_into()? {
            Commands::Offer => {
                let sdp_offer = fields
                    .find_ctx(0)?
                    .utf8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;

                if sdp_offer.trim().is_empty() {
                    return Err(ErrorCode::InvalidCommand.into());
                }

                let (sdp_answer, ice_candidates) = if let Some(ref exchange) = self.sdp_exchange {
                    match exchange(sdp_offer) {
                        Ok((answer, candidates)) => {
                            debug!(
                                sdp_answer_len = answer.len(),
                                candidates = candidates.len(),
                                "Offer — SDP exchanged via media backend"
                            );
                            (answer, candidates)
                        }
                        Err(e) => {
                            tracing::warn!(
                                err = %e,
                                "Offer — media-backend SDP exchange failed"
                            );
                            return Err(ErrorCode::Failure.into());
                        }
                    }
                } else {
                    tracing::warn!("Offer — no SDP exchanger configured");
                    return Err(ErrorCode::InvalidState.into());
                };

                let mut writer = reply.with_command(Commands::OfferResponse as _)?;
                let tag = writer.tag().clone();
                let mut tw = writer.writer();
                tw.start_struct(&tag)?;
                tw.utf8(&TLVTag::Context(0), &sdp_answer)?;
                tw.start_array(&TLVTag::Context(1))?;
                for candidate in &ice_candidates {
                    tw.utf8(&TLVTag::Anonymous, candidate)?;
                }
                tw.end_container()?;
                tw.end_container()?;
                drop(tw);
                writer.complete()
            }
            Commands::Answer => {
                let sdp_answer = fields
                    .find_ctx(0)?
                    .utf8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;
                debug!(sdp_len = sdp_answer.len(), "Answer accepted (no-op)");
                Ok(())
            }
            Commands::ICECandidate => {
                debug!("ICECandidate accepted (no-op)");
                Ok(())
            }
            Commands::End => {
                let reason = fields
                    .find_ctx(0)
                    .ok()
                    .and_then(|e| e.u8().ok())
                    .unwrap_or(0);
                debug!(reason, "End accepted (no-op)");
                Ok(())
            }
            Commands::OfferResponse => Err(ErrorCode::CommandNotFound.into()),
        }
    }
}

impl NonBlockingHandler for WebRtcRequestorHandler {}
