use rubble::bytes::ByteReader;
use rubble::link::CompanyId;

use crate::govee::Error::Irrelevant;

#[derive(Debug, Eq, PartialEq)]
pub struct ClimateReadings {
    temperature: i32,
    humidity: u32,
    battery: u8,
}

#[derive(Debug)]
pub enum Error {
    ParseError,
    Irrelevant,
}

const SENSOR_COMPANY_ID: CompanyId = CompanyId::from_raw(0xEC88);

pub fn parse_payload(company_id: CompanyId, payload: &[u8]) -> Result<ClimateReadings, Error> {
    if company_id != SENSOR_COMPANY_ID {
        return Err(Irrelevant);
    }

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
                (temp_hum_raw / 100) as i32
            } else {
                (temp_hum_raw ^ 0x800000) as i32 / -100
            };
            let humidity = temp_hum_raw % 1000;
            Ok(ClimateReadings {
                temperature: temp,
                humidity,
                battery,
            })
        }
        7 => {
            // Govee H5074
            let mut bytes = ByteReader::new(payload);
            bytes.skip(1).unwrap();
            let temp_bytes: [u8; 2] = bytes.read_array().unwrap();
            let temp_raw = i16::from_le_bytes(temp_bytes);
            let temp = temp_raw;
            let humidity_raw = bytes.read_u16_le().unwrap();
            let humidity = humidity_raw / 10;
            let battery = bytes.read_u8().unwrap();
            Ok(ClimateReadings {
                temperature: i32::from(temp),
                humidity: u32::from(humidity),
                battery,
            })
        }
        _ => Err(Error::Irrelevant),
    }
}

impl ClimateReadings {
    pub fn temperature(&self) -> f32 {
        self.temperature as f32 / 100.
    }

    pub fn humidity(&self) -> f32 {
        self.humidity as f32 / 10.
    }

    pub fn battery(&self) -> u8 {
        self.battery
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_h5075() {
        let payload = &[0x00, 0x03, 0x7c, 0x8a, 0x37, 0x00];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 2284,
            humidity: 490,
            battery: 55,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5075_2() {
        // This data came from https://github.com/Thrilleratplay/GoveeWatcher
        // 88ec00 03519e 64 00 Temp: 21.7502°C Temp: 71.1504°F Humidity: 50.2%
        let payload = &[0x00, 0x03, 0x51, 0x9e, 0x64, 0x00];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 2175,
            humidity: 502,
            battery: 100,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5075_3() {
        // This data came from https://github.com/asednev/govee-bt-client/blob/66a5027838c66d3ae16ee037616989410061789d/src/decode.spec.ts#L26
        // const hex = "88ec000368d15800";
        // const expectedReading = {
        //     battery: 88,
        //     humidity: 44.1,
        //     tempInC: 22.3441,
        //     tempInF: 72.21938,
        // };
        let payload = &[0x00, 0x03, 0x68, 0xd1, 0x58, 0x00];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 2234,
            humidity: 441,
            battery: 88,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5074() {
        let payload = &[0x00, 0x1b, 0x09, 0xf1, 0x18, 0x64, 0x02];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 2331,
            humidity: 638,
            battery: 100,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5074_2() {
        let payload = &[0x00, 0x71, 0x05, 0xd7, 0x14, 0x64, 0x02];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 1393,
            humidity: 533,
            battery: 100,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5074_3() {
        // This data came from https://github.com/neilsheps/GoveeTemperatureAndHumidity
        // 88EC00 0902 CD15 64 02 (Temp) 5.21°C (Humidity) 55.81% (Battery) 100%
        let payload = &[0x00, 0x09, 0x02, 0xCD, 0x15, 0x64, 0x02];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: 521,
            humidity: 558,
            battery: 100,
        };
        assert_eq!(readings, expected);
    }

    #[test]
    fn test_parse_h5074_negative() {
        let payload = &[0x00, 0x23, 0xf9, 0x33, 0x1a, 0x5e, 0x02];
        let readings = parse_payload(SENSOR_COMPANY_ID, payload).unwrap();
        let expected = ClimateReadings {
            temperature: -1757,
            humidity: 670,
            battery: 94,
        };
        assert_eq!(readings, expected);
    }

    // TODO: Add test for negative temperatures from H2075
}
