#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]

use nrf52840_hal as hal;
use nrf_data_logger as _; // global logger + panicking-behavior + memory layout

use core::fmt::Write;
use core::str;

use enc28j60::smoltcp_phy::Phy;
use enc28j60::Enc28j60;
use hal::gpio::Level;
use hal::gpio::{p0, p1};
use hal::Delay;
use hal::Spim;
use hal::Timer as HalTimer;
use rubble::link::filter::AddressFilter;
use rubble::link::AddressKind;
use smoltcp::iface::{EthernetInterface, EthernetInterfaceBuilder, Neighbor, NeighborCache};
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

use nrf_data_logger::ethernet;
use nrf_data_logger::ethernet::{EthPhy, IP_ADDRESS, MAC_ADDRESS};
use shared::{bluetooth, govee};
use smoltcp::socket::{SocketSet, TcpSocket, TcpSocketBuffer};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv6Cidr};

const INDOOR_SENSOR: DeviceAddress =
    DeviceAddress::new([0x24, 0xBE, 0x59, 0x38, 0xC1, 0xA4], AddressKind::Public); // A4:C1:38:59:BE:24 H5075
const OUTDOOR_SENSOR: DeviceAddress =
    DeviceAddress::new([0x4E, 0xEC, 0x50, 0x3C, 0x37, 0xE3], AddressKind::Public); // E3:37:3C:50:EC:4E H5074
const DEVICE_ADDRESSES: &[DeviceAddress] = &[INDOOR_SENSOR, OUTDOOR_SENSOR];

const NET_BUF_SIZE: usize = 1024;

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
        ethernet: EthernetInterface<'static, &'static mut EthPhy>,
        #[init(0)]
        timer_ms: u32,
    }

    #[init(resources = [ble_tx_buf, ble_rx_buf])]
    fn init(ctx: init::Context) -> init::LateResources {
        static mut RX_BUFFER: [u8; NET_BUF_SIZE] = [0; NET_BUF_SIZE];
        static mut TX_BUFFER: [u8; NET_BUF_SIZE] = [0; NET_BUF_SIZE];
        static mut NEIGHBOR_STORAGE: [Option<(IpAddress, Neighbor)>; 16] = [None; 16];
        static mut ETH: Option<EthPhy> = None;
        static mut IP_ADDRS: [IpCidr; 1] = [IpCidr::Ipv6(Ipv6Cidr::SOLICITED_NODE_PREFIX)];

        // On reset, the internal high frequency clock is already used, but we
        // also need to switch to the external HF oscillator. This is needed
        // for Bluetooth to work.
        let _clocks = hal::clocks::Clocks::new(ctx.device.CLOCK).enable_ext_hfosc();

        // SPI
        let port0 = p0::Parts::new(ctx.device.P0);
        let port1 = p1::Parts::new(ctx.device.P1);
        let spiclk = port0.p0_02.into_push_pull_output(Level::Low).degrade();
        let spimosi = port0.p0_26.into_push_pull_output(Level::Low).degrade();
        let spimiso = port0.p0_27.into_floating_input().degrade();

        let pins = hal::spim::Pins {
            sck: spiclk,
            miso: Some(spimiso),
            mosi: Some(spimosi),
        };
        let spi = Spim::new(
            ctx.device.SPIM2,
            pins,
            hal::spim::Frequency::K500,
            hal::spim::MODE_0,
            0,
        );

        // ENC28J60
        let enc28j60 = {
            let ncs = port1.p1_15.into_push_pull_output(Level::High);
            let reset = port1.p1_14.into_push_pull_output(Level::High);
            let mut delay = Delay::new(ctx.core.SYST);

            Enc28j60::new(
                spi,
                ncs,
                enc28j60::Unconnected,
                reset,
                &mut delay,
                7168,
                ethernet::MAC_ADDRESS,
            )
            .ok()
            .unwrap()
        };

        // PHY Wrapper
        let eth = Phy::new(enc28j60, &mut RX_BUFFER[..], &mut TX_BUFFER[..]);
        ETH.replace(eth);

        // Ethernet interface
        let ip_addr = IpCidr::new(IpAddress::from(IP_ADDRESS), 24);
        *IP_ADDRS = [ip_addr];
        let neighbor_cache = NeighborCache::new(&mut NEIGHBOR_STORAGE[..]);
        let ethernet_addr = EthernetAddress(MAC_ADDRESS);
        let iface = EthernetInterfaceBuilder::new(ETH.as_mut().unwrap())
            .ethernet_addr(ethernet_addr)
            .ip_addrs(&mut IP_ADDRS[..])
            .neighbor_cache(neighbor_cache)
            .finalize();

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

        let _ = HalTimer::periodic(ctx.device.TIMER1);

        init::LateResources {
            radio,
            scanner,
            timer,
            ethernet: iface,
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

    #[task(binds = TIMER1, resources = [timer_ms])]
    fn timer1(ctx: timer1::Context) {
        let timer_ms = ctx.resources.timer_ms;
        *timer_ms += 1;
    }

    #[idle(resources = [ethernet, timer_ms])]
    fn idle(mut c: idle::Context) -> ! {
        // let stim0 = &mut c.resources.itm.stim[0];
        let iface = c.resources.ethernet;

        // Copied / modified from smoltcp:
        // examples/loopback.rs
        let echo_socket = {
            static mut TCP_SERVER_RX_DATA: [u8; 1024] = [0; 1024];
            static mut TCP_SERVER_TX_DATA: [u8; 1024] = [0; 1024];
            let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
            let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
            TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
        };
        let greet_socket = {
            static mut TCP_SERVER_RX_DATA: [u8; 256] = [0; 256];
            static mut TCP_SERVER_TX_DATA: [u8; 256] = [0; 256];
            let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
            let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
            TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
        };
        let mut socket_set_entries = [None, None];
        let mut socket_set = SocketSet::new(&mut socket_set_entries[..]);
        let echo_handle = socket_set.add(echo_socket);
        let greet_handle = socket_set.add(greet_socket);
        // {
        //     let store = unsafe { &mut NET_STORE };
        //     defmt::info!("TCP sockets will listen at {}", store.ip_addrs[0].address());
        // }

        // // Copied / modified from:
        // // smoltcp:examples/loopback.rs, examples/server.rs;
        // // stm32-eth:examples/ip.rs,
        // // git.m-labs.hk/M-Labs/tnetplug
        loop {
            // Poll
            // TODO: Configure a timer or use SysTick to produce a timer
            let now = c.resources.timer_ms.lock(|v| *v);
            let instant = Instant::from_millis(now as i64);
            match iface.poll(&mut socket_set, instant) {
                Ok(_) => {}
                Err(_e) => {
                    defmt::error!("[{}] Poll error", instant.millis)
                }
            }
            // Control the "echoing" socket (:1234)
            {
                let mut socket = socket_set.get::<TcpSocket<'_>>(echo_handle);
                if !socket.is_open() {
                    defmt::info!(
                        "[{}] Listening to port 1234 for echoing, time-out in 10s",
                        instant.millis
                    );
                    socket.listen(1234).unwrap();
                    socket.set_timeout(Some(smoltcp::time::Duration::from_millis(10000)));
                }
                if socket.can_recv() {
                    if let Ok(packet) =
                        socket.recv(|buffer| (buffer.len(), str::from_utf8(buffer).unwrap()))
                    {
                        defmt::info!("[{}] Received packet: {}", instant.millis, packet);
                    } else {
                        defmt::error!("[{}] Received packet error", instant.millis);
                    }
                }
            }
            // Control the "greeting" socket (:4321)
            {
                let mut socket = socket_set.get::<TcpSocket<'_>>(greet_handle);
                if !socket.is_open() {
                    defmt::info!(
                        "[{}] Listening to port 4321 for greeting, \
                        please connect to the port",
                        instant.millis
                    );
                    socket.listen(4321).unwrap();
                }
                if socket.can_send() {
                    let greeting = "🦀 Rust powered TCP/IP working on nRF52840";
                    write!(socket, "{}\n", greeting).unwrap();
                    defmt::info!("[{}] Greeting sent, socket closed", instant.millis);
                    socket.close();
                }
            }
        }
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
                        defmt::info!("Manufacturer specific data: {} - Temp: {}℃, Humidity: {}%, Battery: {}%" , addr_str, readings.temperature(), readings.humidity(), readings.battery());
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
