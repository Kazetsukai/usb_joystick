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
    let piezo = Output::new(AnyPin::from(p.PIN_18), Level::Low);

    let _ = spawner.spawn(blinker(led, Duration::from_millis(500)));
    let _ = spawner.spawn(play_seq(piezo));
}

static SEQ: [u32; 8] = [440, 494, 523, 587, 659, 698, 784, 880];
static SOMETHING: [u32; 16] = [
    659, 659, 659, 587, 659, 698, 494, 587, 523, 440, 494, 523, 440, 659, 784, 659,
];

#[embassy_executor::task]
async fn play_seq(pin: Output<'static, AnyPin>) {
    let mut play_pin = pin;
    for freq in SOMETHING {
        play_pin = play_tone(play_pin, Duration::from_millis(200), freq * 8).await;
        Timer::after(Duration::from_millis(100)).await;
    }
}

async fn play_tone(
    mut pin: Output<'static, AnyPin>,
    duration: Duration,
    freq: u32,
) -> Output<'static, AnyPin> {
    let period = Duration::from_micros(1_000_000 / freq as u64);
    let count = (duration.as_micros() / period.as_micros()) as u32;
    let halfperiod = period / 2;

    info!(
        "Playing {:?}hz for {:?}ms ({:?} cycles)",
        freq,
        duration.as_millis(),
        count
    );

    for _ in 0..count {
        pin.set_high();
        Timer::after(halfperiod).await;
        pin.set_low();
        Timer::after(halfperiod).await;
    }

    return pin;
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
