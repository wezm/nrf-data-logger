#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

use core::{char, str};

use nrf52840_hal as hal;
use nrf_data_logger as _; // global logger + panicking-behavior + memory layout

use rubble::link::AddressKind;
use {
    rubble::{
        beacon::{BeaconScanner, ScanCallback},
        link::{ad_structure::AdStructure, filter::AllowAll, DeviceAddress, MIN_PDU_BUF},
        time::{Duration, Timer},
    },
    rubble_nrf5x::{
        radio::{BleRadio, PacketBuffer},
        timer::BleTimer,
    },
};

pub struct BeaconScanCallback;

impl ScanCallback for BeaconScanCallback {
    fn beacon<'a, I>(&mut self, addr: DeviceAddress, _data: I)
    where
        I: Iterator<Item = AdStructure<'a>>,
    {
        // Detected an advertisement frame! Do something with it here.
        let kind = match addr.kind() {
            AddressKind::Public => "public",
            AddressKind::Random => "random",
        };
        let mut data = [0u8; 12];
        for (i, byte) in addr.raw().iter().copied().enumerate() {
            data[i] = char::from_digit((u32::from(byte) & 0xF0) >> 4, 16).unwrap() as u8;
            data[i + 1] = char::from_digit(u32::from(byte) & 0xF, 16).unwrap() as u8;
        }
        let addr_str = str::from_utf8(&data).unwrap();
        defmt::info!("Got advertisement frame from address {} {}", kind, addr_str);
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
        scanner: BeaconScanner<BeaconScanCallback, AllowAll>,
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
        let mut scanner = BeaconScanner::new(BeaconScanCallback);
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
