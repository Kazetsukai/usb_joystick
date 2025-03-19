#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod hid_descriptor;
mod joystick;
mod network;
mod state;
mod usb_device;
mod usb_ethernet;
mod web;

static DEVICE_NAME: &str = "Custom Joystick";
static DEVICE_HOST: &str = "joystick";

static OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 42, 0, 1);
static DNS_SERVERS: [Ipv4Addr; 1] = [OUR_IP];

const MTU: usize = 1514;

use {
    core::net::Ipv4Addr,
    defmt::info,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::{
        adc, bind_interrupts,
        clocks::RoscRng,
        gpio::{AnyPin, Level, Output},
        i2c::InterruptHandler,
        peripherals::{I2C1, USB},
        usb::{self, Driver},
    },
    embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex},
    embassy_time::{Duration, Timer},
    embassy_usb::{class::cdc_ncm::embassy_net::Device, UsbDevice},
    joystick::JoystickRunner,
    panic_probe as _,
    picoserve::make_static,
    rand::RngCore,
    state::AppState,
};

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    ADC_IRQ_FIFO => adc::InterruptHandler;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(AnyPin::from(p.PIN_22), Level::Low);

    let shared_state = make_static!(
        state::SharedState,
        state::SharedState(make_static!(Mutex<CriticalSectionRawMutex, bool>, Mutex::new(true)))
    );

    Timer::after_millis(100).await;

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    let usb_driver = Driver::new(p.USB, Irqs);

    //spawner.must_spawn(usb_device::be_usb_device(spawner, shared_state));

    let mut builder = usb_device::get_usb_builder(usb_driver);
    let (ncm_runner, device) = usb_ethernet::make_usb_ethernet_device(&mut builder);
    let (net_runner, stack) = network::make_network_stack(device, seed);
    let (joystick_runner, hid_runner) = joystick::make_joystick(
        &mut builder,
        embassy_rp::adc::Adc::new(p.ADC, Irqs, embassy_rp::adc::Config::default()),
        p.PIN_26,
        p.PIN_27,
        p.PIN_28,
        p.PIN_20,
        p.PIN_21,
        p.PIN_2,
        p.PIN_3,
        p.PIN_4,
        p.PIN_5,
        p.PIN_6,
        p.PIN_7,
    );
    let usb = builder.build();
    let (app, config) = web::make_web_app();

    spawner.must_spawn(blinker(led, Duration::from_millis(500)));

    spawner.must_spawn(usb_task(usb));
    info!("USB task started");

    spawner.must_spawn(usb_ncm_task(ncm_runner));
    info!("USB NCM task started");

    spawner.must_spawn(network::net_task(net_runner));
    info!("Network task started");

    // Spawn network service tasks
    spawner.must_spawn(network::dhcp_task(stack));
    info!("DHCP server task started");

    spawner.must_spawn(network::mdns_task(stack));
    info!("mDNS server task started");

    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web::web_task(
            id,
            stack,
            AppState {
                power_state: *shared_state,
            },
            app,
            config,
        ));
    }
    info!("Web task started");

    spawner.must_spawn(hid_task(hid_runner));
    info!("HID task started");

    spawner.must_spawn(joystick_task(joystick_runner));
    info!("Joystick task started");

    loop {
        Timer::after(Duration::from_secs(3)).await;
    }
}

#[embassy_executor::task]
async fn blinker(mut led: Output<'static>, interval: Duration) {
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

#[embassy_executor::task]
pub(crate) async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
pub(crate) async fn usb_ncm_task(
    class: embassy_usb::class::cdc_ncm::embassy_net::Runner<'static, Driver<'static, USB>, MTU>,
) -> ! {
    class.run().await
}

#[embassy_executor::task]
pub(crate) async fn net_task(mut runner: embassy_net::Runner<'static, Device<'static, MTU>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn hid_task(runner: joystick::HidResponderRunner<'static, Driver<'static, USB>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn joystick_task(mut runner: JoystickRunner<Driver<'static, USB>>) -> ! {
    runner.run().await
}
