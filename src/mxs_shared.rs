// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                           MXS Protocol
// —————————————————————————————————————————————————————————————————————————————————————————————————

/// MXS Protocol (MiXed Stream Protocol)
///
/// A simple protocol for extracting structured packets from mixed ASCII/binary data streams.
/// Commonly used for serial/USB communications where debug output and structured data coexist.
/// No CRC performed
///
/// Packet Structure:
/// [MARKER:2][TYPE:1][LENGTH 0:1]
/// [MARKER:2][TYPE:1][LENGTH N:1][DATA:N]
///
///  
pub const MARKER: &[u8] = &[0xAA, 0x55];

pub const MARKER_LEN: usize = MARKER.len();
pub const TYPE_LEN: usize = 1;
pub const SIZE_LEN: usize = 1;

pub const MAX_DATA_LEN: usize = (1usize << (SIZE_LEN * 8)) - 1;
pub const MIN_PACKET_SIZE: usize = MARKER_LEN + TYPE_LEN + SIZE_LEN;
pub const MAX_PACKET_SIZE: usize = MARKER_LEN + TYPE_LEN + SIZE_LEN + MAX_DATA_LEN;

/// Protocol Packet Types
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum MxsPacketType {
    Start     = 1,
    End       = 2,
    Heartbeat = 3,
    Data      = 4,
    Error     = 5,
}

impl TryFrom<u8> for MxsPacketType {
    type Error = ();

    fn try_from(value: u8) -> core::result::Result<Self, <MxsPacketType as TryFrom<u8>>::Error> {
        match value {
            v if v == Self::Start as u8 => Ok(Self::Start),
            v if v == Self::End as u8 => Ok(Self::End),
            v if v == Self::Heartbeat as u8 => Ok(Self::Heartbeat),
            v if v == Self::Data as u8 => Ok(Self::Data),
            v if v == Self::Error as u8 => Ok(Self::Error),
            _ => Err(()),
        }
    }
}
