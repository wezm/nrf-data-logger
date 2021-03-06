#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

use nrf52840_hal as hal;
use nrf_data_logger as _; // global logger + panicking-behavior + memory layout

use rubble::link::filter::AddressFilter;
use rubble::link::AddressKind;
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

use shared::{bluetooth, govee};

macro_rules! addr {
    ($b1:literal : $b2:literal : $b3:literal : $b4:literal : $b5:literal : $b6:literal) => {
        [$b6, $b5, $b4, $b3, $b2, $b1]
    };
}

// A4:C1:38:59:BE:24 GVH5075_BE24 -- Indoor
const GVH5075_BE24: DeviceAddress =
    DeviceAddress::new(addr!(0xA4:0xC1:0x38:0x59:0xBE:0x24), AddressKind::Public);
// E3:37:3C:50:EC:4E Govee_H5074_EC4E -- Outdoor
const GOVEE_H5074_EC4E: DeviceAddress =
    DeviceAddress::new(addr!(0xE3:0x37:0x3C:0x50:0xEC:0x4E), AddressKind::Public);
// E0:17:54:D0:A8:ED Govee_H5074_A8ED
const GOVEE_H5074_A8ED: DeviceAddress =
    DeviceAddress::new(addr!(0xE0:0x17:0x54:0xD0:0xA8:0xED), AddressKind::Public);
// E3:60:59:42:EC:C1 Govee_H5074_ECC1
const GOVEE_H5074_ECC1: DeviceAddress =
    DeviceAddress::new(addr!(0xE3:0x60:0x59:0x42:0xEC:0xC1), AddressKind::Public);
const DEVICE_ADDRESSES: &[DeviceAddress] = &[
    GVH5075_BE24,
    GOVEE_H5074_EC4E,
    GOVEE_H5074_A8ED,
    GOVEE_H5074_ECC1,
];

pub struct BeaconScanCallback;
pub struct HomeDeviceFilter;

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

impl AddressFilter for HomeDeviceFilter {
    fn matches(&self, address: DeviceAddress) -> bool {
        DEVICE_ADDRESSES
            .iter()
            .copied()
            .any(|device| address == device)
    }
}

impl ScanCallback for BeaconScanCallback {
    fn beacon<'a, I>(&mut self, addr: DeviceAddress, data: I)
    where
        I: Iterator<Item = AdStructure<'a>>,
    {
        // Detected an advertisement frame! Do something with it here.
        let mut buf = [0; 12 + 5];
        let addr_str = bluetooth::fmt_addr(addr.raw(), &mut buf);
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
                    company_identifier,
                    payload,
                } => match govee::parse_payload(company_identifier, payload) {
                    Ok(readings) => {
                        defmt::info!("Manufacturer specific data: {} - Temp: {}???, Humidity: {}%, Battery: {}%" , addr_str, readings.temperature(), readings.humidity(), readings.battery());
                    }
                    Err(govee::Error::ParseError) => {
                        defmt::error!("payload parse error")
                    }
                    Err(govee::Error::Irrelevant) => {}
                },
                AdStructure::Unknown { ty: 8, .. } => {
                    // defmt::info!(
                    //     "Shortened local name {}",
                    //     str::from_utf8(data).unwrap_or("not utf-8")
                    // )
                }
                AdStructure::Unknown { ty: 9, .. } => {
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
