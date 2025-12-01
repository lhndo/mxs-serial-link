pub use crate::mxs_shared::*;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            MXS Encoder
// —————————————————————————————————————————————————————————————————————————————————————————————————

// For Embedded
use heapless::Vec as HVec;

pub struct MxsEncoder;

impl MxsEncoder {
    #[inline]
    pub fn create_data_package(p_type: MxsPacketType, data: &[u8]) -> HVec<u8, MAX_PACKET_SIZE> {
        assert!(data.len() <= MAX_DATA_LEN, "Data larger than buffer");

        let mut packet = HVec::<u8, _>::new();
        let data_len = data.len().to_le_bytes();

        packet.extend_from_slice(MARKER).unwrap();
        packet.push(p_type as u8).unwrap();
        packet.extend_from_slice(&data_len[..SIZE_LEN]).unwrap();
        packet.extend_from_slice(data).unwrap();

        packet
    }

    #[inline]
    pub fn create_package(p_type: MxsPacketType) -> HVec<u8, MAX_PACKET_SIZE> {
        Self::create_data_package(p_type, &[])
    }
}
