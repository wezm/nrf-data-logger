#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

use core::{char, str};

use nrf52840_hal as hal;
use nrf_data_logger as _; // global logger + panicking-behavior + memory layout

use rubble::bytes::ByteReader;
use rubble::link::filter::AddressFilter;
use rubble::link::{AddressKind, CompanyId};
use {
    rubble::{
        beacon::{BeaconScanner, ScanCallback},
        link::{ad_structure::AdStructure, DeviceAddress, MIN_PDU_BUF},
        time::{Duration, Timer},
    },
    rubble_nrf5x::{
        radio::{BleRadio, PacketBuffer},
        timer::BleTimer,
    },
};

const SENSOR_COMPANY_ID: CompanyId = CompanyId::from_raw(0xEC88);
const INDOOR_SENSOR: [u8; 6] = [0x24, 0xBE, 0x59, 0x38, 0xC1, 0xA4]; // A4:C1:38:59:BE:24 H5075
const OUTDOOR_SENSOR: [u8; 6] = [0x4E, 0xEC, 0x50, 0x3C, 0x37, 0xE3]; // E3:37:3C:50:EC:4E H5074
const SENSOR_MACS: &[[u8; 6]] = &[INDOOR_SENSOR, OUTDOOR_SENSOR];

pub struct BeaconScanCallback;
pub struct HomeDeviceFilter;

impl ScanCallback for BeaconScanCallback {
    fn beacon<'a, I>(&mut self, addr: DeviceAddress, data: I)
    where
        I: Iterator<Item = AdStructure<'a>>,
    {
        // Detected an advertisement frame! Do something with it here.
        let mut buf = [0; 12 + 5];
        let addr_str = fmt_addr(addr.raw(), &mut buf);
        // defmt::info!("Got advertisement frame from address {}", addr_str);

        for ad in data {
            match ad {
                AdStructure::Flags(_) => {
                    // defmt::info!("Flags")
                }
                AdStructure::ServiceUuids16(_) => {
                    // defmt::info!("ServiceUuids16")
                }
                AdStructure::ServiceUuids32(_) => {
                    // defmt::info!("ServiceUuids32")
                }
                AdStructure::ServiceUuids128(_) => {
                    // defmt::info!("ServiceUuids128")
                }
                AdStructure::ServiceData16 { .. } => {
                    // defmt::info!("ServiceData16")
                }
                AdStructure::CompleteLocalName(_) => {
                    // defmt::info!("CompleteLocalName")
                }
                AdStructure::ShortenedLocalName(_) => {
                    // defmt::info!("ShortenedLocalName")
                }
                AdStructure::ManufacturerSpecificData {
                    company_identifier: SENSOR_COMPANY_ID,
                    payload,
                } => {
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
                            // float((self.packet % 1000) / 10)

                            defmt::info!("Manufacturer specific data: {} - Temp: {}℃, Humidity: {}%, Battery: {}%" , addr_str, temp, humidity, battery);
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

                            defmt::info!("Manufacturer specific data: {} - Temp: {}℃, Humidity: {}%, Battery: {}%" , addr_str, temp, humidity, battery);
                        }
                        _ => {
                            defmt::info!(
                                "Manufacturer specific data: unexpected payload len: {}",
                                payload.len()
                            )
                        }
                    }
                }
                AdStructure::ManufacturerSpecificData {
                    company_identifier,
                    payload,
                } => {
                    defmt::info!(
                        "Manufacturer specific data: CompanyId({:X}), payload len: {}",
                        company_identifier.as_u16(),
                        payload.len()
                    )
                }
                AdStructure::Unknown { ty: 8, data } => {
                    // defmt::info!(
                    //     "Shortened local name {}",
                    //     str::from_utf8(data).unwrap_or("not utf-8")
                    // )
                }
                AdStructure::Unknown { ty: 9, data } => {
                    // defmt::info!(
                    //     "Complete local name {}",
                    //     str::from_utf8(data).unwrap_or("not utf-8")
                    // )
                }
                AdStructure::Unknown { ty, data } => {
                    defmt::info!("Unknown type {}, {} bytes", ty, data.len())
                }
                _ => {
                    defmt::info!("Unknown")
                }
            }
        }
    }
}

impl AddressFilter for HomeDeviceFilter {
    fn matches(&self, address: DeviceAddress) -> bool {
        SENSOR_MACS
            .iter()
            .copied()
            .any(|mac| address == DeviceAddress::new(mac, AddressKind::Public))
    }
}

#[rtic::app(device = crate::hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        #[init([0; MIN_PDU_BUF])]
        ble_tx_buf: PacketBuffer,
        #[init([0; MIN_PDU_BUF])]
        ble_rx_buf: PacketBuffer,
        radio: BleRadio,
        timer: BleTimer<hal::pac::TIMER0>,
        scanner: BeaconScanner<BeaconScanCallback, HomeDeviceFilter>,
    }

    #[init(resources = [ble_tx_buf, ble_rx_buf])]
    fn init(ctx: init::Context) -> init::LateResources {
        // On reset, the internal high frequency clock is already used, but we
        // also need to switch to the external HF oscillator. This is needed
        // for Bluetooth to work.
        let _clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();

        // Initialize BLE timer
        let mut timer = BleTimer::init(ctx.device.TIMER0);

        // Initialize radio
        let mut radio = BleRadio::new(
            ctx.device.RADIO,
            &ctx.device.FICR,
            ctx.resources.ble_tx_buf,
            ctx.resources.ble_rx_buf,
        );

        // Set up beacon scanner for continuous scanning. The advertisement
        // channel that is being listened on (scan window) will be switched
        // every 500 ms.
        let mut scanner = BeaconScanner::with_filter(BeaconScanCallback, HomeDeviceFilter);
        let scanner_cmd = scanner.configure(timer.now(), Duration::from_millis(500));

        // Reconfigure radio and timer
        radio.configure_receiver(scanner_cmd.radio);
        timer.configure_interrupt(scanner_cmd.next_update);

        init::LateResources {
            radio,
            scanner,
            timer,
        }
    }

    #[task(binds = RADIO, resources = [radio, scanner, timer])]
    fn radio(ctx: radio::Context) {
        let timer = ctx.resources.timer;
        let scanner = ctx.resources.scanner;
        let radio = ctx.resources.radio;

        if let Some(next_update) = radio.recv_beacon_interrupt(scanner) {
            timer.configure_interrupt(next_update);
        }
    }

    #[task(binds = TIMER0, resources = [radio, timer, scanner])]
    fn timer0(ctx: timer0::Context) {
        let timer = ctx.resources.timer;
        let scanner = ctx.resources.scanner;
        let radio = ctx.resources.radio;

        // Clear interrupt
        if !timer.is_interrupt_pending() {
            return;
        }
        timer.clear_interrupt();

        // Update scanner (switch to next advertisement channel)
        let cmd = scanner.timer_update(timer.now());
        radio.configure_receiver(cmd.radio);
        timer.configure_interrupt(cmd.next_update);
    }
};

fn fmt_addr<'buf>(addr: &[u8; 6], addr_str: &'buf mut [u8; 12 + 5]) -> &'buf str {
    *addr_str = [b':'; 12 + 5];
    for (i, byte) in addr.iter().copied().rev().enumerate() {
        addr_str[i * 3] = char::from_digit((u32::from(byte) & 0xF0) >> 4, 16)
            .unwrap()
            .to_ascii_uppercase() as u8;
        addr_str[i * 3 + 1] = char::from_digit(u32::from(byte) & 0xF, 16)
            .unwrap()
            .to_ascii_uppercase() as u8;
    }
    str::from_utf8(addr_str).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_addr() {
        let addr = [0x24, 0xBE, 0x59, 0x38, 0xC1, 0xA4];
        let mut buf = [0; 12 + 5];
        let addr_str = fmt_addr(&addr, &mut buf);
        assert_eq!(addr_str, "A4:C1:38:59:BE:24");
    }

    /*
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Key: 0xec88
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Value:
      00 1b 09 f1 18 64 02                             .....d.

     */
}
