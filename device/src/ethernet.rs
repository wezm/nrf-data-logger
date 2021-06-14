use enc28j60::smoltcp_phy::Phy;
use enc28j60::Unconnected;
use nrf52840_hal::gpio::p1::{P1_14, P1_15};
use nrf52840_hal::gpio::{Output, PushPull};
use nrf52840_hal::pac::SPIM2;
use nrf52840_hal::spim::Spim;
use smoltcp::wire::Ipv4Address;

pub const MAC_ADDRESS: [u8; 6] = [0x20, 0x18, 0x03, 0x01, 0x00, 0x00];
pub const IP_ADDRESS: Ipv4Address = Ipv4Address::new(172, 16, 16, 16);

pub type SpiEth = enc28j60::Enc28j60<
    Spim<SPIM2>,
    P1_15<Output<PushPull>>, // NCS
    Unconnected,             // INT
    P1_14<Output<PushPull>>, // RESET
>;

pub type EthPhy = Phy<
    'static,
    Spim<SPIM2>,
    P1_15<Output<PushPull>>, // NCS
    Unconnected,             // INT
    P1_14<Output<PushPull>>, // RESET
>;
