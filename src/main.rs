#![no_std]
#![no_main]

use {
    defmt::*,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::gpio::{AnyPin, Level, Output},
    embassy_time::{Duration, Timer},
    panic_probe as _,
};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(AnyPin::from(p.PIN_19), Level::Low);

    let _ = spawner.spawn(blinker(led, Duration::from_millis(500)));
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
