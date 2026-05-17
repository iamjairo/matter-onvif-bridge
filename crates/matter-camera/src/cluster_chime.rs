//! Chime cluster (0x0556) — Matter 1.5 §9.13
//!
//! Stub implementation that reports zero installed chime sounds. The cluster is
//! required on camera endpoints but this bridge has no physical chime hardware,
//! so all commands return `UnsupportedCommand`.

use rs_matter::attributes;
use rs_matter::commands;
use rs_matter::dm::{
    Access, Attribute, Cluster, Command, Dataver, Handler, InvokeContext, InvokeReply,
    NonBlockingHandler, Quality, ReadContext, ReadReply, Reply, WriteContext,
};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::tlv::TLVWrite;
use strum::FromRepr;

pub const CLUSTER_ID: u32 = 0x0556;
const CLUSTER_REVISION: u16 = 1;

/// Matter 1.5 Chime attribute IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Attributes {
    InstalledChimeSounds = 0x0000,
    ActiveChimeID = 0x0002,
}

rs_matter::attribute_enum!(Attributes);

/// Matter 1.5 Chime command IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Commands {
    PlayChimeSound = 0x00,
}

rs_matter::command_enum!(Commands);

pub const CHIME_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    0, // feature map: no features
    attributes!(
        Attribute::new(
            Attributes::InstalledChimeSounds as _,
            Access::RV,
            Quality::FIXED
        ),
        Attribute::new(Attributes::ActiveChimeID as _, Access::RV, Quality::NONE)
    ),
    commands!(Command::new(
        Commands::PlayChimeSound as _,
        None,
        Access::WO
    )),
    &[],            // events: none
    |_, _, _| true, // with_attrs
    |_, _, _| true, // with_cmds
    |_, _, _| true, // with_events
);

/// Stub cluster handler — no chime hardware available.
pub struct ChimeHandler {
    dataver: Dataver,
}

impl ChimeHandler {
    pub fn new(dataver: Dataver) -> Self {
        Self { dataver }
    }
}

impl Handler for ChimeHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        if let Some(mut writer) = reply.with_dataver(self.dataver.get())? {
            if attr.is_system() {
                return CHIME_CLUSTER.read(attr, writer);
            }

            match attr.attr_id.try_into()? {
                Attributes::InstalledChimeSounds => {
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::ActiveChimeID => writer.set(0u8),
            }
        } else {
            Ok(())
        }
    }

    fn write(&self, _ctx: impl WriteContext) -> Result<(), Error> {
        Err(ErrorCode::AttributeNotFound.into())
    }

    fn invoke(&self, _ctx: impl InvokeContext, _reply: impl InvokeReply) -> Result<(), Error> {
        Err(ErrorCode::CommandNotFound.into())
    }
}

impl NonBlockingHandler for ChimeHandler {}
