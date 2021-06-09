use rubble::bytes::ByteReader;

pub struct ClimateReadings {
    pub temperature: f32,
    pub humidity: f32,
    pub battery: u8,
}

pub enum Error {
    ParseError,
    UnknownPayloadLength
}

pub fn parse_payload(payload: &[u8]) -> Result<ClimateReadings, Error> {
    match payload.len() {
        6 => {
            // Govee H5072/H5075
            let mut bytes = ByteReader::new(payload);
            bytes.skip(1).unwrap();
            let mut temp_hum: [u8; 4] = bytes.read_array().unwrap();
            let battery = temp_hum[3];
            temp_hum[3] = 0;
            let temp_hum_raw = u32::from_be_bytes(temp_hum) >> 8;

            // casts are safe because temp_hum_raw is only 3 bytes
            let temp = if temp_hum_raw & 0x800000 == 0 {
                (temp_hum_raw) as f32 / 10_000.
            } else {
                (temp_hum_raw ^ 0x800000) as f32 / -10_000.
            };
            let humidity = (temp_hum_raw % 1000) as f32 / 10.;
            Ok(ClimateReadings {
                temperature: temp,
                humidity: humidity,
                battery,
            })
        }
        7 => {
            // Govee H5074
            let mut bytes = ByteReader::new(payload);
            bytes.skip(1).unwrap();
            let temp_bytes: [u8; 2] = bytes.read_array().unwrap();
            let temp_raw = i16::from_le_bytes(temp_bytes);
            let temp = f32::from(temp_raw) / 100.;
            let humidity_raw = bytes.read_u16_le().unwrap();
            let humidity = f32::from(humidity_raw) / 100.;
            let battery = bytes.read_u8().unwrap();
            Ok(ClimateReadings {
                temperature: temp,
                humidity: humidity,
                battery,
            })
        }
        _ => {
            Err(Error::UnknownPayloadLength)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Key: 0xec88
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Value:
      00 1b 09 f1 18 64 02                             .....d.

     */
}
