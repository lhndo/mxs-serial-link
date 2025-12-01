pub use crate::mxs_shared::*;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            MXS Decoder
// —————————————————————————————————————————————————————————————————————————————————————————————————

/// Decoded Packet
#[derive(Debug)]
pub struct MxsPacket<'a> {
    pub packet_type: MxsPacketType,
    pub data:        &'a [u8],
}

#[derive(Debug)]
pub struct MxsFilterResult<'a> {
    pub skipped_data: &'a [u8],
    pub trim_index:   usize,
    pub packets:      Vec<MxsPacket<'a>>,
}

pub struct MxsDecoder<'a> {
    data:     &'a [u8],
    cursor:   usize,
    skip_pos: Option<usize>,
}

impl<'a> MxsDecoder<'a> {
    /// Zero-copy packet filter.
    ///
    /// Scans the input buffer, extracts all complete packets, and returns:
    /// - the bytes skipped before the first valid packet,
    /// - the index of the last valid processed byte (for buffer draining),
    /// - a list of decoded packets (empty if none were found).
    ///
    /// The caller can trim the processed portion of the buffer and append new data
    /// before calling this function again. No allocations occur beyond the packet list.
    #[inline]
    pub fn filter_buffer(data: &'a [u8]) -> MxsFilterResult<'a> {
        let mut decoder = Self {
            data,
            cursor: 0,
            skip_pos: None,
        };
        let mut packets = Vec::new();

        while let Some(packet) = decoder.extract_packet() {
            packets.push(packet);
        }

        let first_pos = decoder.skip_pos.unwrap_or(0);
        let skipped_data = &data[..first_pos];
        let trim_index = { if packets.is_empty() { first_pos } else { decoder.cursor } };

        MxsFilterResult {
            skipped_data,
            trim_index,
            packets,
        }
    }

    #[inline]
    fn extract_packet(&mut self) -> Option<MxsPacket<'a>> {
        // Buffer too short to be able to extract a marker
        if self.cursor + MARKER_LEN > self.data.len() {
            return None;
        }

        // ---- Find packet start
        let found_rel = self.data[self.cursor..]
            .windows(MARKER_LEN)
            .position(|w| w == MARKER);

        // We skip most of the data if no markers were found in the entire buffer
        if found_rel.is_none() {
            if self.skip_pos.is_none() {
                self.skip_pos = Some(self.data.len() - (MARKER_LEN - 1));
            }
            return None;
        }

        let start_pos = found_rel.unwrap() + self.cursor;

        // Check if we have enough data to extract a minimal packet
        if start_pos + MIN_PACKET_SIZE > self.data.len() {
            // skip the non matching data
            if self.skip_pos.is_none() {
                self.skip_pos = Some(start_pos);
            }
            return None;
        }

        let type_pos = start_pos + MARKER_LEN;

        // ---- Extract Packet Type
        let packet_type = match MxsPacketType::try_from(self.data[type_pos]) {
            Ok(pt) => pt,
            Err(_) => {
                // Unknown Packet Type or false marker
                // Skip the non matching data
                let skip_pos = start_pos + MARKER_LEN;
                if self.skip_pos.is_none() {
                    self.skip_pos = Some(skip_pos);
                }
                self.cursor = skip_pos;

                return None;
            }
        };

        // ---- Extract Data Length
        let size_pos = type_pos + TYPE_LEN;
        let data_len = self.data[size_pos] as usize;

        // ---- Extract Data
        let data_start = size_pos + SIZE_LEN;
        let data_end = data_start + data_len;

        // Ensure packet fits in buffer
        if data_end > self.data.len() {
            self.cursor = start_pos; // Buffer too short, exit
            // skip the non matching data
            if self.skip_pos.is_none() {
                self.skip_pos = Some(start_pos);
            }
            return None;
        }

        let payload = &self.data[data_start..data_end];
        self.cursor = data_end;

        // Track first packet position
        if self.skip_pos.is_none() {
            self.skip_pos = Some(start_pos);
        }

        Some(MxsPacket {
            packet_type,
            data: payload,
        })
    }
}
