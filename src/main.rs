#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]

mod hid_descriptor;
mod state;
mod usb_device;

static DEVICE_NAME: &str = "Custom Joystick";
static DEVICE_HOST: &str = "joystick";

static OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 42, 0, 1);
static DNS_SERVERS: [Ipv4Addr; 1] = [OUR_IP];

use {
    core::{convert, net::Ipv4Addr},
    defmt::*,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::{
        adc, bind_interrupts,
        gpio::{AnyPin, Level, Output},
        i2c::{self, Config, InterruptHandler},
        peripherals::{ADC, I2C1, USB},
        usb,
    },
    embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex},
    embassy_time::{Duration, Timer},
    embedded_hal_async::i2c::I2c,
    panic_probe as _,
    static_cell::make_static,
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

    let _ = spawner.spawn(blinker(led, Duration::from_millis(500)));

    let shared_state = make_static!(state::SharedState(make_static!(Mutex::new(true))));

    Timer::after_millis(100).await;

    spawner
        .spawn(usb_device::be_usb_device(
            spawner,
            p.USB,
            shared_state,
            p.ADC,
            p.PIN_26,
            p.PIN_27,
            p.PIN_28,
            p.PIN_20,
            p.PIN_21,
        ))
        .unwrap();
}

#[embassy_executor::task]
async fn blinker(mut led: Output<'static, AnyPin>, interval: Duration) {
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}
