//! Slave Information Interface (SII).

use crate::{
    all_consumed,
    base_data_types::PrimitiveDataType,
    error::{EepromError, Error},
    sync_manager_channel::{self},
};
use ethercrab_wire::{EtherCrabWire, EtherCrabWireSized};
use nom::{
    bytes::complete::take,
    combinator::{map, map_opt, map_res},
    number::complete::{le_i16, le_u16, le_u8},
    IResult,
};

pub const TX_PDO_RANGE: core::ops::RangeInclusive<u16> = 0x1A00..=0x1bff;
pub const RX_PDO_RANGE: core::ops::RangeInclusive<u16> = 0x1600..=0x17ff;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, ethercrab_wire::EtherCrabWire)]
#[repr(u8)]
pub enum SiiOwner {
    /// EEPROM access rights are assigned to PDI during state change from Init to PreOp, Init to
    /// Boot and while in Boot
    #[default]
    Master = 0x00,

    /// EEPROM access rights are assigned to PDI in all states except Init
    Pdi = 0x01,
}

/// Defined in ETG1000.4 6.4.3
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, ethercrab_wire::EtherCrabWire)]
#[wire(bytes = 2)]
pub struct SiiControl {
    // First byte, but second octet because little endian
    #[wire(bits = 1)]
    pub access: SiiAccess,
    // reserved4: u8,
    #[wire(pre_skip = 4, bits = 1)]
    pub emulate_sii: bool,
    #[wire(bits = 1)]
    pub read_size: SiiReadSize,
    #[wire(bits = 1)]
    pub address_type: SiiAddressSize,

    #[wire(bits = 1)]
    pub read: bool,
    #[wire(bits = 1)]
    pub write: bool,
    #[wire(bits = 1)]
    pub reload: bool,
    #[wire(bits = 1)]
    pub checksum_error: bool,
    #[wire(bits = 1)]
    pub device_info_error: bool,
    #[wire(bits = 1)]
    pub command_error: bool,
    #[wire(bits = 1)]
    pub write_error: bool,
    #[wire(bits = 1)]
    pub busy: bool,
}

impl SiiControl {
    pub fn has_error(&self) -> bool {
        self.checksum_error || self.device_info_error || self.command_error || self.write_error
    }

    pub fn error_reset(self) -> Self {
        Self {
            checksum_error: false,
            device_info_error: false,
            command_error: false,
            write_error: false,
            ..self
        }
    }

    fn read() -> Self {
        Self {
            read: true,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, ethercrab_wire::EtherCrabWire)]
#[repr(u8)]
pub enum SiiAccess {
    #[default]
    ReadOnly = 0x00,
    ReadWrite = 0x01,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, ethercrab_wire::EtherCrabWire)]
#[repr(u8)]
pub enum SiiReadSize {
    /// Read 4 octets at a time.
    #[default]
    Octets4 = 0x00,

    /// Read 8 octets at a time.
    Octets8 = 0x01,
}

impl SiiReadSize {
    pub fn chunk_len(&self) -> u16 {
        match self {
            SiiReadSize::Octets4 => 4,
            SiiReadSize::Octets8 => 8,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, ethercrab_wire::EtherCrabWire)]
#[repr(u8)]
pub enum SiiAddressSize {
    #[default]
    U8 = 0x00,
    U16 = 0x01,
}

#[derive(ethercrab_wire::EtherCrabWire)]
#[wire(bytes = 6)]
pub struct SiiRequest {
    #[wire(bytes = 2)]
    control: SiiControl,
    // Post skip is required to send the correct amount of bytes on the wire. This is weird because
    // addressing is all a single WORD, but the SII read request expects a low AND high WORD, hence
    // the extra 16 bits of padding here for the unusedhigh WORD.
    #[wire(bytes = 2, post_skip = 16)]
    address: u16,
}

impl core::fmt::Debug for SiiRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SiiRequest")
            .field("control", &self.control)
            .field("address", &format_args!("{:#06x}", self.address))
            .finish()
    }
}

impl SiiRequest {
    pub fn read(address: u16) -> Self {
        Self {
            control: SiiControl::read(),
            address,
        }
    }
}

/// SII register address.
///
/// Defined in ETG1000.6 Table 16 or ETG2010 Table 2
#[derive(Debug, Copy, Clone, ethercrab_wire::EtherCrabWire)]
#[repr(u16)]
pub enum SiiCoding {
    /// PDI Control
    // Unsigned16
    PdiControl = 0x0000,
    /// PDI Configuration
    // Unsigned16
    PdiConfiguration = 0x0001,
    /// SyncImpulseLen
    // Unsigned16
    SyncImpulseLen = 0x0002,
    /// PDI Configuration2
    ///
    /// Initialization value for PDI Configuration register R8 most significant word (0x152-0x153)
    // Unsigned16
    PdiConfiguration2 = 0x0003,
    /// Configured Station Alias
    // Unsigned16
    ConfiguredStationAlias = 0x0004,
    /// Checksum
    // Unsigned16
    Checksum = 0x0007,
    /// Vendor ID
    // Unsigned32
    VendorId = 0x0008,
    /// Product Code
    // Unsigned32
    ProductCode = 0x000A,
    /// Revision Number
    // Unsigned32
    RevisionNumber = 0x000C,
    /// Serial Number
    // Unsigned32
    SerialNumber = 0x000E,
    /// Reserved
    // BYTE
    Reserved = 0x0010,
    /// Bootstrap Receive Mailbox Offset
    // Unsigned16
    BootstrapReceiveMailboxOffset = 0x0014,
    /// Bootstrap Receive Mailbox Size
    // Unsigned16
    BootstrapReceiveMailboxSize = 0x0015,
    /// Bootstrap Send Mailbox Offset
    // Unsigned16
    BootstrapSendMailboxOffset = 0x0016,
    /// Bootstrap Send Mailbox Size
    // Unsigned16
    BootstrapSendMailboxSize = 0x0017,
    /// Standard Receive Mailbox Offset
    // Unsigned16
    StandardReceiveMailboxOffset = 0x0018,
    /// Standard Receive Mailbox Size
    // Unsigned16
    StandardReceiveMailboxSize = 0x0019,
    /// Standard Send Mailbox Offset
    // Unsigned16
    StandardSendMailboxOffset = 0x001A,
    /// Standard Send Mailbox Size
    // Unsigned16
    StandardSendMailboxSize = 0x001B,
    /// Mailbox Protocol - returns a [`MailboxProtocols`](crate::mailbox::MailboxProtocols).
    // Unsigned16
    MailboxProtocol = 0x001C,
    /// Size
    // Unsigned16
    Size = 0x003E,
    /// Version
    // Unsigned16
    Version = 0x003F,
}

/// Defined in ETG1000.6 Table 19.
///
/// Additional information also in ETG1000.6 Table 17.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, ethercrab_wire::EtherCrabWire)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u16)]
pub enum CategoryType {
    #[default]
    Nop = 0,
    #[wire(alternatives = [2,3,4,5,6,7,8,9])]
    DeviceSpecific = 1,
    Strings = 10,
    DataTypes = 20,
    General = 30,
    Fmmu = 40,
    SyncManager = 41,
    FmmuExtended = 42,
    SyncUnit = 43,
    TxPdo = 50,
    RxPdo = 51,
    DistributedClock = 60,
    // Device specific: 0x1000-0xfffe
    End = 0xffff,
}

/// The type of PDO to search for.
#[derive(Debug, Copy, Clone)]
pub enum PdoType {
    Tx = 50,
    Rx = 51,
}

impl From<PdoType> for CategoryType {
    fn from(value: PdoType) -> Self {
        match value {
            PdoType::Tx => Self::TxPdo,
            PdoType::Rx => Self::RxPdo,
        }
    }
}

/// ETG1000.6 Table 23
#[derive(Debug, Copy, Clone, PartialEq, Eq, ethercrab_wire::EtherCrabWire)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum FmmuUsage {
    #[wire(alternatives = [0xff])]
    Unused = 0x00,
    Outputs = 0x01,
    Inputs = 0x02,
    SyncManagerStatus = 0x03,
}

impl FromEeprom for FmmuUsage {
    const STORAGE_SIZE: usize = 1;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, usage) = map_res(take(1usize), FmmuUsage::unpack_from_slice)(i)?;

        Ok((i, usage))
    }
}

/// ETG1020 Table 10 "FMMU_EX"
///
/// NOTE: Most fields defined are discarded from this struct as they are unused in Ethercrab.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FmmuEx {
    /// Sync manager index.
    pub sync_manager: u8,
}

impl FromEeprom for FmmuEx {
    const STORAGE_SIZE: usize = 3;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, _becore) = le_u8(i)?;
        let (i, sync_manager) = le_u8(i)?;
        let (i, _after) = le_u8(i)?;

        all_consumed(i)?;

        Ok((i, Self { sync_manager }))
    }
}

/// SII "General" category.
///
/// Defined in ETG1000.6 Table 21
#[derive(Debug, PartialEq, Eq)]
pub struct SiiGeneral {
    pub(crate) group_string_idx: u8,
    pub(crate) image_string_idx: u8,
    pub(crate) order_string_idx: u8,
    pub name_string_idx: u8,
    // reserved: u8,
    pub coe_details: CoeDetails,
    pub(crate) foe_enabled: bool,
    pub(crate) eoe_enabled: bool,
    // Following 3 fields marked as reserved
    // soe_channels: u8,
    // ds402_channels: u8,
    // sysman_class: u8,
    pub(crate) flags: Flags,
    /// EBus Current Consumption in mA.
    ///
    /// A negative Values means feeding in current feed in sets the available current value to the
    /// given value
    pub(crate) ebus_current: i16,
    // reserved: u8,
    pub(crate) ports: [PortStatus; 4],
    /// defines the ESC memory address where the Identification ID is saved if Identification Method
    /// [`IDENT_PHY_M`] is set.
    pub(crate) physical_memory_addr: u16,
    // reserved2: [u8; 12]
}

impl FromEeprom for SiiGeneral {
    const STORAGE_SIZE: usize = 16;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, group_string_idx) = le_u8(i)?;
        let (i, image_string_idx) = le_u8(i)?;
        let (i, order_string_idx) = le_u8(i)?;
        let (i, name_string_idx) = le_u8(i)?;
        let (i, _reserved) = le_u8(i)?;
        let (i, coe_details) = map_opt(le_u8, CoeDetails::from_bits)(i)?;
        let (i, foe_enabled) = map(le_u8, |num| num != 0)(i)?;
        let (i, eoe_enabled) = map(le_u8, |num| num != 0)(i)?;

        // Reserved, ignored
        let (i, _soe_channels) = le_u8(i)?;
        let (i, _ds402_channels) = le_u8(i)?;
        let (i, _sysman_class) = le_u8(i)?;

        let (i, flags) = map_opt(le_u8, Flags::from_bits)(i)?;
        let (i, ebus_current) = le_i16(i)?;

        let (i, ports) = map(le_u16, |raw| {
            let p1 = raw & 0x0f;
            let p2 = (raw >> 4) & 0x0f;
            let p3 = (raw >> 8) & 0x0f;
            let p4 = (raw >> 12) & 0x0f;

            [
                PortStatus::from(p1 as u8),
                PortStatus::from(p2 as u8),
                PortStatus::from(p3 as u8),
                PortStatus::from(p4 as u8),
            ]
        })(i)?;

        // let (i, physical_memory_addr) = le_u16(i)?;
        let physical_memory_addr = 0;

        all_consumed(i)?;

        Ok((
            i,
            Self {
                group_string_idx,
                image_string_idx,
                order_string_idx,
                name_string_idx,
                coe_details,
                foe_enabled,
                eoe_enabled,
                flags,
                ebus_current,
                ports,
                physical_memory_addr,
            },
        ))
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, ethercrab_wire::EtherCrabWire)]
#[repr(u8)]
pub enum PortStatus {
    #[default]
    Unused = 0x00,
    Mii = 0x01,
    Reserved = 0x02,
    Ebus = 0x03,
    FastHotConnect = 0x04,
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Eq)]
    pub struct Flags: u8 {
        const ENABLE_SAFE_OP = 0x01;
        const ENABLE_NOT_LRW = 0x02;
        const MAILBOX_DLL = 0x04;
        const IDENT_AL_STATUS = 0x08;
        const IDENT_PHY_M = 0x10;

    }
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Eq)]
    pub struct CoeDetails: u8 {
        /// Bit 0: Enable SDO
        const ENABLE_SDO = 0x01;
        /// Bit 1: Enable SDO Info
        const ENABLE_SDO_INFO = 0x02;
        /// Bit 2: Enable PDO Assign
        const ENABLE_PDO_ASSIGN = 0x04;
        /// Bit 3: Enable PDO Configuration
        const ENABLE_PDO_CONFIG = 0x08;
        /// Bit 4: Enable Upload at startup
        const ENABLE_STARTUP_UPLOAD = 0x10;
        /// Bit 5: Enable SDO complete access
        const ENABLE_COMPLETE_ACCESS = 0x20;
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SyncManager {
    pub(crate) start_addr: u16,
    pub(crate) length: u16,
    pub(crate) control: sync_manager_channel::Control,
    pub(crate) enable: SyncManagerEnable,
    pub(crate) usage_type: SyncManagerType,
}

impl core::fmt::Debug for SyncManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SyncManager")
            .field("start_addr", &format_args!("{:#06x}", self.start_addr))
            .field("length", &format_args!("{:#06x}", self.length))
            .field("control", &self.control)
            .field("enable", &self.enable)
            .field("usage_type", &self.usage_type)
            .finish()
    }
}

impl FromEeprom for SyncManager {
    const STORAGE_SIZE: usize = 8;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, start_addr) = le_u16(i)?;
        let (i, length) = le_u16(i)?;
        let (i, control) = map_res(le_u8, |byte| {
            sync_manager_channel::Control::unpack_from_slice(&[byte])
        })(i)?;

        // Ignored
        let (i, _status) = le_u8(i)?;

        let (i, enable) = map_opt(le_u8, SyncManagerEnable::from_bits)(i)?;
        let (i, usage_type) = map(le_u8, SyncManagerType::from)(i)?;

        all_consumed(i)?;

        Ok((
            i,
            Self {
                start_addr,
                length,
                control,
                enable,
                usage_type,
            },
        ))
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct SyncManagerEnable: u8 {
        /// Bit 0: enable.
        const ENABLE = 0x01;
        /// Bit 1: fixed content (info for config tool –SyncMan has fixed content).
        const IS_FIXED = 0x02;
        /// Bit 2: virtual SyncManager (virtual SyncMan – no hardware resource used).
        const IS_VIRTUAL = 0x04;
        /// Bit 3: opOnly (SyncMan should be enabled only in OP state).
        const OP_ONLY = 0x08;
    }
}

// Can't derive, so manual impl
#[cfg(feature = "defmt")]
impl defmt::Format for SyncManagerEnable {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=u8:b}", self.bits())
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, ethercrab_wire::EtherCrabWire)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum SyncManagerType {
    /// Not used or unknown.
    #[default]
    Unknown = 0x00,
    /// Used for writing into the slave.
    MailboxWrite = 0x01,
    /// Used for reading from the slave.
    MailboxRead = 0x02,
    /// Used for process data outputs from master.
    ProcessDataWrite = 0x03,
    /// Used for process data inputs to master.
    ProcessDataRead = 0x04,
}

/// Defined in ETG2010 Table 14 – Structure Category TXPDO and RXPDO for each PDO
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Pdo {
    pub(crate) index: u16,
    pub(crate) num_entries: u8,
    pub(crate) sync_manager: u8,
    pub(crate) dc_sync: u8,
    /// Index into EEPROM Strings section for PDO name.
    pub(crate) name_string_idx: u8,
    pub(crate) flags: PdoFlags,
    pub(crate) entries: heapless::Vec<PdoEntry, 16>,
}

impl core::fmt::Debug for Pdo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pdo")
            .field("index", &format_args!("{:#06x}", self.index))
            .field("num_entries", &self.num_entries)
            .field("sync_manager", &self.sync_manager)
            .field("dc_sync", &self.dc_sync)
            .field("name_string_idx", &self.name_string_idx)
            .field("flags", &self.flags)
            .field("entries", &self.entries)
            .finish()
    }
}

impl Pdo {
    /// Compute the total bit length of this PDO by iterating over and summing the bit length of
    /// each entry contained within.
    pub fn bit_len(&self) -> u16 {
        self.entries
            .iter()
            .map(|entry| u16::from(entry.data_length_bits))
            .sum()
    }
}

impl FromEeprom for Pdo {
    const STORAGE_SIZE: usize = 8;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, index) = le_u16(i)?;
        let (i, num_entries) = le_u8(i)?;
        let (i, sync_manager) = le_u8(i)?;
        let (i, dc_sync) = le_u8(i)?;
        let (i, name_string_idx) = le_u8(i)?;
        let (i, flags) = map_opt(le_u16, PdoFlags::from_bits)(i)?;

        all_consumed(i)?;

        Ok((
            i,
            Self {
                index,
                num_entries,
                sync_manager,
                dc_sync,
                name_string_idx,
                flags,
                entries: heapless::Vec::new(),
            },
        ))
    }
}

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PdoEntry {
    pub(crate) index: u16,
    pub(crate) sub_index: u8,
    pub(crate) name_string_idx: u8,
    // See page 103 of ETG2000
    pub(crate) data_type: PrimitiveDataType,
    pub(crate) data_length_bits: u8,
    pub(crate) flags: u16,
}

impl core::fmt::Debug for PdoEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PdoEntry")
            .field("index", &format_args!("{:#06x}", self.index))
            .field("sub_index", &self.sub_index)
            .field("name_string_idx", &self.name_string_idx)
            .field("data_type", &self.data_type)
            .field("data_length_bits", &self.data_length_bits)
            .field("flags", &self.flags)
            .finish()
    }
}

impl FromEeprom for PdoEntry {
    const STORAGE_SIZE: usize = 8;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, index) = le_u16(i)?;
        let (i, sub_index) = le_u8(i)?;
        let (i, name_string_idx) = le_u8(i)?;
        let (i, data_type) = map_res(
            take(PrimitiveDataType::PACKED_LEN),
            PrimitiveDataType::unpack_from_slice,
        )(i)?;
        let (i, data_length_bits) = le_u8(i)?;
        let (i, flags) = le_u16(i)?;

        all_consumed(i)?;

        Ok((
            i,
            Self {
                index,
                sub_index,
                name_string_idx,
                data_type,
                data_length_bits,
                flags,
            },
        ))
    }
}

bitflags::bitflags! {
    /// Defined in ETG2010 Table 14 offset 0x0006.
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct PdoFlags: u16 {
        /// PdoMandatory [Esi:RTxPdo@Mandatory]
        const PDO_MANDATORY = 0x0001;
        /// PdoDefault [Esi:RTxPdo@Sm]
        const PDO_DEFAULT = 0x0002;
        /// Reserved (PdoOversample)
        const PDO_OVERSAMPLE = 0x0004;
        /// PdoFixedContent [Esi:RTxPdo@Fixed]
        const PDO_FIXED_CONTENT = 0x0010;
        /// PdoVirtualContent [Esi:RTxPdo@Virtual]
        const PDO_VIRTUAL_CONTENT = 0x0020;
        /// Reserved (PdoDownloadAnyway)
        const PDO_DOWNLOAD_ANYWAY = 0x0040;
        /// Reserved (PdoFromModule)
        const PDO_FROM_MODULE = 0x0080;
        /// PdoModuleAlign [Esi:Slots:ModulePdoGroup@Alignment]
        const PDO_MODULE_ALIGN = 0x0100;
        /// PdoDependOnSlot [Esi:RTxPdo:Index@DependOnSlot]
        const PDO_DEPEND_ON_SLOT = 0x0200;
        /// PdoDependOnSlotGroup [Esi:RTxPdo:Index@DependOnSlotGroup]
        const PDO_DEPEND_ON_SLOT_GROUP = 0x0400;
        /// PdoOverwrittenByModule [Esi:RTxPdo@OverwrittenByModule]
        const PDO_OVERWRITTEN_BY_MODULE = 0x0800;
        /// Reserved (PdoConfigurable)
        const PDO_CONFIGURABLE = 0x1000;
        /// Reserved (PdoAutoPdoName)
        const PDO_AUTO_PDO_NAME = 0x2000;
        /// Reserved (PdoDisAutoExclude)
        const PDO_DIS_AUTO_EXCLUDE = 0x4000;
        /// Reserved (PdoWritable)
        const PDO_WRITABLE = 0x8000;
    }
}

// Can't derive, so manual impl
#[cfg(feature = "defmt")]
impl defmt::Format for PdoFlags {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=u16:b}", self.bits())
    }
}

bitflags::bitflags! {
    /// Supported mailbox category.
    ///
    /// Defined in ETG1000.6 Table 18 or ETG2010 Table 4.
    #[derive(Copy, Clone, Default, Debug, PartialEq)]
    pub struct MailboxProtocols: u16 {
        /// ADS over EtherCAT (routing and parallel services).
        const AOE = 0x0001;
        /// Ethernet over EtherCAT (tunnelling of Data Link services).
        const EOE = 0x0002;
        /// CAN application protocol over EtherCAT (access to SDO).
        const COE = 0x0004;
        /// File Access over EtherCAT.
        const FOE = 0x0008;
        /// Servo Drive Profile over EtherCAT.
        const SOE = 0x0010;
        /// Vendor specific protocol over EtherCAT.
        const VOE = 0x0020;
    }
}

// Can't derive, so manual impl
#[cfg(feature = "defmt")]
impl defmt::Format for MailboxProtocols {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=u16:b}", self.bits())
    }
}

#[derive(Copy, Clone, Default, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DefaultMailbox {
    /// Master to slave receive mailbox address offset.
    pub slave_receive_offset: u16,
    /// Master to slave receive mailbox size.
    pub slave_receive_size: u16,
    /// Slave to master send mailbox address offset.
    pub slave_send_offset: u16,
    /// Slave to master send mailbox size.
    pub slave_send_size: u16,
    /// Mailbox protocols supported by the slave device.
    pub supported_protocols: MailboxProtocols,
}

impl DefaultMailbox {
    pub fn has_mailbox(&self) -> bool {
        !self.supported_protocols.is_empty() && self.slave_receive_size > 0
            || self.slave_send_size > 0
    }
}

impl FromEeprom for DefaultMailbox {
    const STORAGE_SIZE: usize = 10;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, receive_offset) = le_u16(i)?;
        let (i, receive_size) = le_u16(i)?;
        let (i, send_offset) = le_u16(i)?;
        let (i, send_size) = le_u16(i)?;
        let (i, supported_protocols) = map_opt(le_u16, MailboxProtocols::from_bits)(i)?;

        all_consumed(i)?;

        Ok((
            i,
            Self {
                slave_receive_offset: receive_offset,
                slave_receive_size: receive_size,
                slave_send_offset: send_offset,
                slave_send_size: send_size,
                supported_protocols,
            },
        ))
    }
}

impl core::fmt::Debug for DefaultMailbox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MailboxConfig")
            .field(
                "slave_receive_offset",
                &format_args!("{:#06x}", self.slave_receive_offset),
            )
            .field(
                "slave_receive_size",
                &format_args!("{:#06x}", self.slave_receive_size),
            )
            .field(
                "slave_send_offset",
                &format_args!("{:#06x}", self.slave_send_offset),
            )
            .field(
                "slave_send_size",
                &format_args!("{:#06x}", self.slave_send_size),
            )
            .field("supported_protocols", &self.supported_protocols)
            .finish()
    }
}

pub trait FromEeprom: Sized {
    const STORAGE_SIZE: usize;

    fn parse_fields(i: &[u8]) -> IResult<&[u8], Self>;

    fn parse(i: &[u8]) -> Result<Self, Error> {
        Self::parse_fields(i)
            .map(|(_rest, parsed)| parsed)
            .map_err(|_e| {
                #[cfg(feature = "std")]
                crate::fmt::error!("EEPROM object {:?} {}", core::any::type_name::<Self>(), _e);

                Error::Eeprom(EepromError::Decode)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sii_control_pack() {
        let ctl = SiiControl {
            access: SiiAccess::ReadWrite,
            emulate_sii: false,
            read_size: SiiReadSize::Octets8,
            address_type: SiiAddressSize::U8,
            read: false,
            write: false,
            reload: false,
            checksum_error: false,
            device_info_error: false,
            command_error: false,
            write_error: false,
            busy: true,
        };

        assert_eq!(ctl.pack(), [0b0100_0001, 0b1000_0000],);
    }

    #[test]
    fn sii_request_read_pack() {
        let packed = SiiRequest::read(0x1234).pack();

        assert_eq!(packed, [0x00, 0x01, 0x34, 0x12, 0x00, 0x00]);
    }
}
