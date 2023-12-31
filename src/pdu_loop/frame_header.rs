//! An EtherCAT frame.

use crate::{
    error::{Error, PduError},
    parse::{map_res, new_le_u16},
    LEN_MASK,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, num_enum::FromPrimitive, num_enum::IntoPrimitive)]
#[repr(u8)]
enum ProtocolType {
    DlPdu = 0x01u8,
    NetworkVariables = 0x04,
    Mailbox = 0x05,
    #[num_enum(catch_all)]
    Unknown(u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct FrameHeader(pub u16);

impl FrameHeader {
    /// Create a new PDU frame header.
    pub fn pdu(len: u16) -> Self {
        debug_assert!(
            len <= LEN_MASK,
            "Frame length may not exceed {} bytes",
            LEN_MASK
        );

        let len = len & LEN_MASK;

        let protocol_type = u16::from(u8::from(ProtocolType::DlPdu)) << 12;

        Self(len | protocol_type)
    }

    /// Remove and parse an EtherCAT frame header from the given buffer.
    pub fn parse(i: &[u8]) -> Result<(&[u8], Self), Error> {
        map_res(new_le_u16(i)?, |raw| {
            let header = Self(raw);

            if header.protocol_type() == ProtocolType::DlPdu {
                Ok(header)
            } else {
                Err(Error::Pdu(PduError::Decode))
            }
        })
    }

    /// The length of the payload contained in this frame.
    pub fn payload_len(&self) -> usize {
        usize::from(self.0 & LEN_MASK)
    }

    fn protocol_type(&self) -> ProtocolType {
        let raw = (self.0 >> 12) as u8 & 0b1111;

        raw.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdu_header() {
        let header = FrameHeader::pdu(0x28);

        let packed = header.0;

        let expected = 0b0001_0000_0010_1000;

        assert_eq!(packed, expected, "{packed:016b} == {expected:016b}");
    }

    #[test]
    fn decode_pdu_len() {
        let raw = 0b0001_0000_0010_1000;

        let header = FrameHeader(raw);

        assert_eq!(header.payload_len(), 0x28);
        assert_eq!(header.protocol_type(), ProtocolType::DlPdu);
    }

    #[test]
    fn parse() {
        // Header from packet #39, soem-slaveinfo-ek1100-only.pcapng
        let raw = &[0x3c, 0x10];

        let (rest, header) = FrameHeader::parse(raw).unwrap();

        assert_eq!(rest, &[]);

        assert_eq!(header.payload_len(), 0x3c);
        assert_eq!(header.protocol_type(), ProtocolType::DlPdu);
    }
}
