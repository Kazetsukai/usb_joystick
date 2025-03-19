use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};
use defmt::info;

use edge_dhcp::io::{self, DEFAULT_SERVER_PORT};
use edge_dhcp::server::{Server, ServerOptions};
use edge_mdns::buf::VecBufAccess;
use edge_mdns::domain::base::Ttl;
use edge_mdns::host::Host;
use edge_mdns::io::{Mdns, IPV4_DEFAULT_SOCKET};
use edge_mdns::HostAnswersMdnsHandler;
use edge_nal::{UdpBind, UdpSplit};
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_net::{Ipv4Address, Stack};
use embassy_rp::clocks::RoscRng;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use rand::RngCore;

use crate::{DEVICE_HOST, DNS_SERVERS, OUR_IP};

const MTU: usize = 1514;

#[embassy_executor::task]
pub async fn dhcp_task(stack: Stack<'static>) -> () {
    let mut buf = [0; 1500];

    let buffers: UdpBuffers<1, 1500, 1500, 2> = UdpBuffers::new();
    let udp = Udp::new(stack, &buffers);
    let mut socket = udp
        .bind(SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(0, 0, 0, 0),
            DEFAULT_SERVER_PORT,
        )))
        .await
        .unwrap();

    let options = {
        let mut options = ServerOptions::new(OUR_IP, None);
        options.dns = &DNS_SERVERS;
        options
    };

    return io::server::run(
        &mut Server::<_, 2>::new_with_et(OUR_IP),
        &options,
        &mut socket,
        &mut buf,
    )
    .await
    .unwrap();
}

#[embassy_executor::task]
pub async fn captive_dns_task(stack: Stack<'static>) -> () {
    let mut tx_buf: [u8; 1500] = [0; 1500];
    let mut rx_buf: [u8; 1500] = [0; 1500];
    let ip = Ipv4Addr::new(10, 42, 0, 1);

    let buffers: UdpBuffers<3, 1500, 1500, 2> = UdpBuffers::new();
    let udp = Udp::new(stack, &buffers);

    edge_captive::io::run(
        &udp,
        SocketAddr::new(core::net::IpAddr::V4(ip), 53),
        &mut tx_buf,
        &mut rx_buf,
        ip,
        core::time::Duration::from_secs(60),
    )
    .await
    .unwrap();
}

#[embassy_executor::task]
pub async fn mdns_task(stack: Stack<'static>) -> () {
    let (recv_buf, send_buf) = (
        VecBufAccess::<NoopRawMutex, 1500>::new(),
        VecBufAccess::<NoopRawMutex, 1500>::new(),
    );

    let ip = Ipv4Addr::new(10, 42, 0, 1);

    let buffers: UdpBuffers<3, 1500, 1500, 2> = UdpBuffers::new();
    let udp = Udp::new(stack, &buffers);
    let mut socket = udp.bind(IPV4_DEFAULT_SOCKET).await.unwrap();
    let (recv, send) = socket.split();

    let signal = Signal::<NoopRawMutex, ()>::new();
    let mdns = Mdns::new(
        Some(Ipv4Address::UNSPECIFIED),
        None,
        recv,
        send,
        &recv_buf,
        &send_buf,
        |arr| RoscRng.fill_bytes(arr),
        &signal,
    );

    let host = Host {
        hostname: DEVICE_HOST,
        ipv4: ip,
        ipv6: Ipv6Addr::UNSPECIFIED,
        ttl: Ttl::from_secs(60),
    };

    info!("Starting mDNS server");

    mdns.run(HostAnswersMdnsHandler::new(&host)).await.unwrap();
}

#[embassy_executor::task]
pub async fn net_task(
    mut runner: embassy_net::Runner<
        'static,
        embassy_usb::class::cdc_ncm::embassy_net::Device<'static, MTU>,
    >,
) -> ! {
    runner.run().await
}
