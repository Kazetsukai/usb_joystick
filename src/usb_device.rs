use defmt::info;
use edge_dhcp::Ipv4Addr;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::{
    adc::{Adc, Channel},
    clocks::RoscRng,
    gpio::{Input, Pull},
    peripherals::{
        ADC, PIN_2, PIN_20, PIN_21, PIN_26, PIN_27, PIN_28, PIN_3, PIN_4, PIN_5, PIN_6, PIN_7, USB,
    },
    usb::Driver,
};
use embassy_time::Timer;
use embassy_usb::{
    class::{
        cdc_ncm::{
            self,
            embassy_net::{Device, Runner, State as NetState},
            CdcNcmClass,
        },
        hid::{self, HidReaderWriter},
    },
    UsbDevice,
};

use heapless::Vec;
use picoserve::{
    extract, make_static,
    response::{json, File, IntoResponse},
    routing::{get, get_service},
};
use picoserve::{AppRouter, AppWithStateBuilder};
use rand::RngCore;
use static_cell::StaticCell;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::hid_descriptor::ControlPanelReport;
use crate::state::{AppState, SharedState};
use crate::{joystick, network, web, Irqs, DEVICE_NAME};

const MTU: usize = 1514;
const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const SCRIPT_JS: &str = include_str!("../static/script.js");

struct AppProps;

async fn get_state(
    extract::State(SharedState(leds)): extract::State<SharedState>,
) -> impl IntoResponse {
    json::Json(*leds.lock().await)
}

impl AppWithStateBuilder for AppProps {
    type State = AppState;
    type PathRouter = impl picoserve::routing::PathRouter<AppState>;

    fn build_app(self) -> picoserve::Router<Self::PathRouter, Self::State> {
        picoserve::Router::new()
            .route("/", get_service(File::html(INDEX_HTML)))
            .route("/style.css", get_service(File::css(STYLE_CSS)))
            .route("/script.js", get_service(File::javascript(SCRIPT_JS)))
            .route("/state", get(get_state))
    }
}

#[embassy_executor::task]
pub async fn be_usb_device(
    spawner: Spawner,
    usb: USB,
    state: &'static SharedState,
    adc: ADC,
    pin_vx: PIN_26,
    pin_vy: PIN_27,
    pin_vz: PIN_28,
    pin_s1: PIN_20,
    pin_s2: PIN_21,
    pin_2: PIN_2,
    pin_3: PIN_3,
    pin_4: PIN_4,
    pin_5: PIN_5,
    pin_6: PIN_6,
    pin_7: PIN_7,
) {
    info!("USB device task started");
    let driver = Driver::new(usb, Irqs);
    let mut rng = RoscRng;

    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Kaze");
        config.product = Some(DEVICE_NAME);
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;

        // Required for windows compatibility.
        config.composite_with_iads = true;
        config.device_class = 0xEF;
        config.device_sub_class = 0x02;
        config.device_protocol = 0x01;
        config
    };

    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    let our_mac_addr = [0xe2, 0x58, 0xb1, 0xe7, 0xfb, 0x12];
    let host_mac_addr = [0x82, 0x88, 0x88, 0x88, 0x88, 0x88];

    // Create classes on the builder.
    let cdc_ncm_class = {
        static STATE: StaticCell<cdc_ncm::State> = StaticCell::new();
        let state = STATE.init(cdc_ncm::State::new());
        CdcNcmClass::new(&mut builder, state, host_mac_addr, 64)
    };

    let config = hid::Config {
        report_descriptor: ControlPanelReport::desc(),
        request_handler: None,
        poll_ms: 60,
        max_packet_size: 64,
    };

    let hid = {
        static STATE: StaticCell<hid::State> = StaticCell::new();
        let state = STATE.init(hid::State::new());
        HidReaderWriter::<_, 1, 8>::new(&mut builder, state, config)
    };

    let usb = builder.build();

    spawner.must_spawn(usb_task(usb));
    info!("USB task started");

    static NET_STATE: StaticCell<NetState<MTU, 4, 4>> = StaticCell::new();
    let (runner, device) = cdc_ncm_class
        .into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(NetState::new()), our_mac_addr);

    spawner.must_spawn(usb_ncm_task(runner));
    info!("USB NCM task started");

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 1), 24),
        dns_servers: Vec::new(),
        gateway: None,
    });

    // Generate random seed
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<12>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        device,
        config,
        RESOURCES.init(StackResources::<12>::new()),
        seed,
    );
    stack
        .join_multicast_group(Ipv4Addr::new(224, 0, 0, 251))
        .unwrap();

    spawner.must_spawn(network::net_task(runner));
    info!("Network task started");

    // Setup web app
    let app = make_static!(AppRouter<web::AppProps>, web::AppProps.build_app());
    let web_config = make_static!(
        picoserve::Config<embassy_time::Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(embassy_time::Duration::from_secs(5)),
            read_request: Some(embassy_time::Duration::from_secs(1)),
            write: Some(embassy_time::Duration::from_secs(1)),
        })
        .keep_connection_alive()
    );

    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web::web_task(
            id,
            stack,
            app,
            web_config,
            AppState {
                shared_state: *state,
            },
        ));
    }
    info!("Web task started");

    // Spawn network service tasks
    spawner.must_spawn(network::dhcp_task(stack));
    info!("DHCP server task started");

    spawner.must_spawn(network::mdns_task(stack));
    info!("mDNS server task started");

    // Joystick setup
    let (reader, writer) = hid.split();

    let adc = Adc::new(adc, Irqs, embassy_rp::adc::Config::default());
    let vx_analog = Channel::new_pin(pin_vx, Pull::None);
    let vy_analog = Channel::new_pin(pin_vy, Pull::None);
    let vz_analog = Channel::new_pin(pin_vz, Pull::None);
    let s1 = Input::new(pin_s1, Pull::Up);
    let s2 = Input::new(pin_s2, Pull::Up);

    let led_0 = Output::new(pin_2, Level::Low);
    let led_1 = Output::new(pin_3, Level::Low);
    let led_2 = Output::new(pin_4, Level::Low);
    let led_3 = Output::new(pin_5, Level::Low);
    let led_4 = Output::new(pin_6, Level::Low);
    let led_5 = Output::new(pin_7, Level::Low);

    Timer::after_secs(1).await;

    let joystick_fut = joystick::handle_joystick(
        adc, vx_analog, vy_analog, vz_analog, s1, s2, led_0, led_1, led_2, led_3, led_4, led_5,
        writer,
    );

    let hid_reader_fut = async {
        reader.run(true, &mut joystick::MyRequestHandler {}).await;
    };

    join(joystick_fut, hid_reader_fut).await;
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn usb_ncm_task(class: Runner<'static, Driver<'static, USB>, MTU>) -> ! {
    class.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device<'static, MTU>>) -> ! {
    runner.run().await
}
