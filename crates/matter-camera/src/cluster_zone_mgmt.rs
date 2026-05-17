//! ZoneManagement cluster (0x0550) — Matter 1.5 §9.5
//!
//! In-memory zone CRUD for camera motion detection zones.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

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

pub const CLUSTER_ID: u32 = 0x0550;
const CLUSTER_REVISION: u16 = 1;

const MAX_ZONES: u8 = 10;
const DEFAULT_SENSITIVITY: u8 = 5;

// ── Attribute enum ──

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Attributes {
    MaxZones = 0x0000,
    Zones = 0x0001,
    Sensitivity = 0x0002,
}

rs_matter::attribute_enum!(Attributes);

// ── Command enum ──

#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[repr(u32)]
pub enum Commands {
    CreateZone = 0x00,
    UpdateZone = 0x01,
    DeleteZone = 0x02,
    GetTripWireConfig = 0x03,
}

/// CreateZoneResponse shares command ID 0x00 (generated direction).
const CREATE_ZONE_RESPONSE_ID: u32 = 0x00;

rs_matter::command_enum!(Commands);

// ── Cluster metadata ──

pub const ZONE_MGMT_CLUSTER: Cluster<'static> = Cluster::new(
    CLUSTER_ID,
    CLUSTER_REVISION,
    0, // feature_map
    attributes!(
        Attribute::new(Attributes::MaxZones as _, Access::RV, Quality::FIXED),
        Attribute::new(Attributes::Zones as _, Access::RV, Quality::NONE),
        Attribute::new(Attributes::Sensitivity as _, Access::RWVM, Quality::NONE)
    ),
    commands!(
        Command::new(
            Commands::CreateZone as _,
            Some(CREATE_ZONE_RESPONSE_ID),
            Access::WO
        ),
        Command::new(Commands::UpdateZone as _, None, Access::WO),
        Command::new(Commands::DeleteZone as _, None, Access::WO),
        Command::new(Commands::GetTripWireConfig as _, None, Access::WO)
    ),
    &[],            // events: none
    |_, _, _| true, // with_attrs
    |_, _, _| true, // with_cmds
    |_, _, _| true, // with_events
);

// ── Data structures ──

#[derive(Clone, Debug)]
pub struct Zone {
    pub zone_id: u8,
    pub name: String,
    pub zone_type: u8, // 0=TripWire, 1=Region
}

// ── Handler ──

pub struct ZoneManagementHandler {
    dataver: Dataver,
    zones: Arc<RwLock<Vec<Zone>>>,
    sensitivity: Arc<RwLock<u8>>,
    next_zone_id: Arc<AtomicU8>,
}

impl ZoneManagementHandler {
    pub fn new(dataver: Dataver) -> Self {
        Self {
            dataver,
            zones: Arc::new(RwLock::new(Vec::new())),
            sensitivity: Arc::new(RwLock::new(DEFAULT_SENSITIVITY)),
            next_zone_id: Arc::new(AtomicU8::new(1)),
        }
    }
}

impl Handler for ZoneManagementHandler {
    fn read(&self, ctx: impl ReadContext, reply: impl ReadReply) -> Result<(), Error> {
        let attr = ctx.attr();

        if let Some(mut writer) = reply.with_dataver(self.dataver.get())? {
            if attr.is_system() {
                return ZONE_MGMT_CLUSTER.read(attr, writer);
            }

            match attr.attr_id.try_into()? {
                Attributes::MaxZones => writer.set(MAX_ZONES),
                Attributes::Zones => {
                    let zones = self.zones.read().map_err(|_| ErrorCode::Busy)?;
                    let tag = writer.tag().clone();
                    let mut tw = writer.writer();
                    tw.start_array(&tag)?;
                    for zone in zones.iter() {
                        tw.start_struct(&TLVTag::Anonymous)?;
                        tw.u8(&TLVTag::Context(0), zone.zone_id)?;
                        tw.utf8(&TLVTag::Context(1), &zone.name)?;
                        tw.u8(&TLVTag::Context(2), zone.zone_type)?;
                        tw.end_container()?;
                    }
                    tw.end_container()?;
                    drop(tw);
                    writer.complete()
                }
                Attributes::Sensitivity => {
                    let sensitivity = self.sensitivity.read().map_err(|_| ErrorCode::Busy)?;
                    writer.set(*sensitivity)
                }
            }
        } else {
            Ok(())
        }
    }

    fn write(&self, ctx: impl WriteContext) -> Result<(), Error> {
        let attr = ctx.attr();
        let data = ctx.data();
        attr.check_dataver(self.dataver.get())?;

        match attr.attr_id.try_into()? {
            Attributes::Sensitivity => {
                let value = data.u8()?;
                let mut sensitivity = self.sensitivity.write().map_err(|_| ErrorCode::Busy)?;
                *sensitivity = value;
                self.dataver.changed();
                Ok(())
            }
            _ => Err(ErrorCode::AttributeNotFound.into()),
        }
    }

    fn invoke(&self, ctx: impl InvokeContext, reply: impl InvokeReply) -> Result<(), Error> {
        let cmd = ctx.cmd();
        let data = ctx.data();
        let fields = data.structure()?;

        match cmd.cmd_id.try_into()? {
            Commands::CreateZone => {
                let name = fields
                    .find_ctx(0)?
                    .utf8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;
                let zone_type = fields.find_ctx(1)?.u8().unwrap_or(0);

                let zone_id = self.next_zone_id.fetch_add(1, Ordering::AcqRel);

                let zone = Zone {
                    zone_id,
                    name: name.to_string(),
                    zone_type,
                };

                {
                    let mut zones = self.zones.write().map_err(|_| ErrorCode::Busy)?;
                    zones.push(zone);
                }
                self.dataver.changed();

                debug!(zone_id, "CreateZone");

                let mut writer = reply.with_command(CREATE_ZONE_RESPONSE_ID)?;
                let tag = writer.tag().clone();
                let mut tw = writer.writer();
                tw.start_struct(&tag)?;
                tw.u8(&TLVTag::Context(0), zone_id)?;
                tw.end_container()?;
                drop(tw);
                writer.complete()
            }
            Commands::UpdateZone => {
                let zone_id = fields
                    .find_ctx(0)?
                    .u8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;
                let name = fields
                    .find_ctx(1)?
                    .utf8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;
                let zone_type = fields.find_ctx(2)?.u8().unwrap_or(0);

                let mut zones = self.zones.write().map_err(|_| ErrorCode::Busy)?;
                let zone = zones
                    .iter_mut()
                    .find(|z| z.zone_id == zone_id)
                    .ok_or(ErrorCode::NotFound)?;

                zone.name = name.to_string();
                zone.zone_type = zone_type;
                self.dataver.changed();

                debug!(zone_id, "UpdateZone");
                Ok(())
            }
            Commands::DeleteZone => {
                let zone_id = fields
                    .find_ctx(0)?
                    .u8()
                    .map_err(|_| ErrorCode::InvalidCommand)?;

                let mut zones = self.zones.write().map_err(|_| ErrorCode::Busy)?;
                let before = zones.len();
                zones.retain(|z| z.zone_id != zone_id);
                if zones.len() == before {
                    return Err(ErrorCode::NotFound.into());
                }
                self.dataver.changed();

                debug!(zone_id, "DeleteZone");
                Ok(())
            }
            Commands::GetTripWireConfig => Err(ErrorCode::CommandNotFound.into()),
        }
    }
}

impl NonBlockingHandler for ZoneManagementHandler {}
