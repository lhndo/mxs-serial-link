// Data format and handling

use anyhow::Result as AnyResult;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Data
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[derive(Debug, Default, Clone, Copy)]
pub struct Data(i16, i16, i16);

impl Data {
    pub fn process(&self) -> AnyResult<String> {
        // TODO: do something with data
        //
        Ok(format!("MXS Data: {:?}", &self))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DataError;

impl TryFrom<&[u8]> for Data {
    type Error = DataError;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        if buf.len() != size_of::<Self>() {
            return Err(DataError);
        }

        let data = Self(
            i16::from_le_bytes(buf[0..2].try_into().map_err(|_| DataError)?),
            i16::from_le_bytes(buf[2..4].try_into().map_err(|_| DataError)?),
            i16::from_le_bytes(buf[4..6].try_into().map_err(|_| DataError)?),
        );

        Ok(data)
    }
}
