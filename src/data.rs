// Data format and handling

use anyhow::Result as AnyResult;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Data
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[derive(Debug, Default, Clone, Copy)]
pub struct Data(i16, i16, i16);

impl Data {
    pub fn try_from(buf: &[u8]) -> Option<Self> {
        if buf.len() != size_of::<Self>() {
            return None;
        }

        let data = Self(
            i16::from_le_bytes(buf[0..2].try_into().ok()?),
            i16::from_le_bytes(buf[2..4].try_into().ok()?),
            i16::from_le_bytes(buf[4..6].try_into().ok()?),
        );

        Some(data)
    }
}

// ———————————————————————————————————————— Process Data ———————————————————————————————————————————

pub fn process_data(data: Data) -> AnyResult<()> {
    // TODO: do something with data
    println!("Thread Data: {:?}", data);
    Ok(())
}
