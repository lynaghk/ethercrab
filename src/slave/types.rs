use crate::{
    eeprom::types::{MailboxProtocols, SyncManagerType},
    pdi::PdiSegment,
};
use core::fmt::Debug;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SlaveConfig {
    pub io: IoRanges,
    pub mailbox: MailboxConfig,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MailboxConfig {
    pub(in crate::slave) read: Option<Mailbox>,
    pub(in crate::slave) write: Option<Mailbox>,
    pub(in crate::slave) supported_protocols: MailboxProtocols,
    pub(in crate::slave) coe_sync_manager_types: heapless::Vec<SyncManagerType, 16>,
    pub(in crate::slave) has_coe: bool,
    /// True if Complete Access is supported.
    pub(in crate::slave) complete_access: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Mailbox {
    pub(in crate::slave) address: u16,
    pub(in crate::slave) len: u16,
    pub(in crate::slave) sync_manager: u8,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IoRanges {
    pub input: PdiSegment,
    pub output: PdiSegment,
}
