use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_rp::{
    adc::{Adc, Channel},
    clocks::RoscRng,
    gpio::{AnyPin, Input, Level, Output, Pull},
    peripherals::{ADC, PIN_20, PIN_21, PIN_26, PIN_27, PIN_28, USB},
    usb::Driver,
};
use embassy_time::{Duration, Timer};
use embassy_usb::{
    class::{
        cdc_ncm::{
            self,
            embassy_net::{Device, Runner, State as NetState},
            CdcNcmClass,
        },
        hid::{self, HidReaderWriter, ReportId, RequestHandler},
    },
    control::OutResponse,
    UsbDevice,
};
use heapless::Vec;
use picoserve::{
    extract,
    response::{json, File, IntoResponse},
    routing::{get, get_service},
    Router,
};
use rand::{Rng, RngCore};
use static_cell::{make_static, StaticCell};
use usbd_hid::descriptor::SerializedDescriptor;

use crate::hid_descriptor::ControlPanelReport;
use crate::state::{AppState, SharedState};
use crate::Irqs;

const MTU: usize = 1514;
const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const SCRIPT_JS: &str = include_str!("../static/script.js");

type AppRouter = impl picoserve::routing::PathRouter<AppState>;

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
) {
    info!("USB device task started");
    let driver = Driver::new(usb, Irqs);
    let mut rng = RoscRng;

    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("USB-Ethernet example");
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
    static STACK: StaticCell<Stack<Device<'static, MTU>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<12>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        device,
        config,
        RESOURCES.init(StackResources::<12>::new()),
        seed,
    ));

    spawner.must_spawn(net_task(stack));
    info!("Network task started");

    async fn get_state(
        extract::State(SharedState(leds)): extract::State<SharedState>,
    ) -> impl IntoResponse {
        json::Json(*leds.lock().await)
    }

    fn make_app() -> Router<AppRouter, AppState> {
        picoserve::Router::new()
            .route("/", get_service(File::html(INDEX_HTML)))
            .route("/style.css", get_service(File::css(STYLE_CSS)))
            .route("/script.js", get_service(File::javascript(SCRIPT_JS)))
            .route("/state", get(get_state))
    }

    let app = make_static!(make_app());

    let config = make_static!(picoserve::Config::new(picoserve::Timeouts {
        start_read_request: Some(Duration::from_secs(5)),
        read_request: Some(Duration::from_secs(1)),
        write: Some(Duration::from_secs(1)),
    })
    .keep_connection_alive());

    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(
            id,
            stack,
            app,
            config,
            AppState {
                shared_state: *state,
            },
        ));
    }

    let (reader, mut writer) = hid.split();
    let mut rng = RoscRng;

    let mut adc = Adc::new(adc, Irqs, embassy_rp::adc::Config::default());
    let mut vx_analog = Channel::new_pin(pin_vx, Pull::None);
    let mut vy_analog = Channel::new_pin(pin_vy, Pull::None);
    let mut vz_analog = Channel::new_pin(pin_vz, Pull::None);
    let mut s1 = Input::new(pin_s1, Pull::Up);
    let mut s2 = Input::new(pin_s2, Pull::Up);

    Timer::after_secs(1).await;

    let in_fut = async {
        loop {
            _ = Timer::after_millis(1).await;
            let report = ControlPanelReport {
                x: -((adc.read(&mut vy_analog).await.unwrap_or_default() / 16) as i16 - 128) as i8,
                y: ((adc.read(&mut vx_analog).await.unwrap_or_default() / 16) as i16 - 128) as i8,
                x2: -((adc.read(&mut vz_analog).await.unwrap_or_default() / 16) as i16 - 128) as i8,
                y2: 0,
                s1: if s1.is_low() { 0 } else { 255 },
                s2: if s2.is_low() { 0 } else { 255 },
            };
            // Send the report.
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            }
        }
    };
    let out_fut = async {
        reader.run(true, &mut MyRequestHandler {}).await;
    };

    join(in_fut, out_fut).await;
}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

const WEB_TASK_POOL_SIZE: usize = 3;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: &'static Stack<Device<'static, MTU>>,
    app: &'static Router<AppRouter, AppState>,
    config: &'static picoserve::Config<Duration>,
    state: AppState,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve_with_state(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
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
async fn net_task(stack: &'static Stack<Device<'static, MTU>>) -> ! {
    stack.run().await
}
