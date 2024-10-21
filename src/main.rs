#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]

mod hid_descriptor;
mod state;
mod usb_device;

use {
    core::convert,
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

    // let sda = p.PIN_2;
    // let scl = p.PIN_3;

    // info!("set up i2c ");
    // let config = {
    //     let mut config = Config::default();
    //     config.frequency = 400_000;
    //     config
    // };
    // let mut i2c = i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, config);
    // const ADDR: u8 = 0x3c; // default addr
    // const ADC_ADDR: u8 = 0x48;

    // const CONFIG_REGISTER: u8 = 0x01;
    // const CONVERSION_REGISTER: u8 = 0x00;
    // const CONFIG_BYTES: [u8; 3] = [CONFIG_REGISTER, 0b0_100_001_0, 0b111_0_0_0_11];

    // const COMMAND_MODE: u8 = 0x00;
    // const DATA_MODE: u8 = 0x40;

    // const DISPLAY_ON: u8 = 0xaf;
    // const DISPLAY_OFF: u8 = 0xae;
    // const DISPLAY_NORMAL: u8 = 0xa6;
    // const DISPLAY_INVERT: u8 = 0xa7;
    // const DISPLAY_ALL_ON: u8 = 0xa5;
    // const DISPLAY_ALL_OFF: u8 = 0xa4;

    // let mut send_blank_page: [u8; 129] = [0; 129];
    // send_blank_page[0] = DATA_MODE;

    // Timer::after(Duration::from_millis(500)).await;

    // info!("set MUX ratio");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xa8, 0x3f]).await.unwrap();
    // info!("set display offset");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xd3, 0x00]).await.unwrap();
    // info!("set display start line");
    // i2c.write(ADDR, &[COMMAND_MODE, 0x40]).await.unwrap();
    // info!("set segment remap");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xa1]).await.unwrap();
    // info!("set COM output scan direction");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xc8]).await.unwrap();
    // info!("set COM pins hardware configuration");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xda, 0x12]).await.unwrap();
    // info!("set contrast control");
    // i2c.write(ADDR, &[COMMAND_MODE, 0x81, 0x7f]).await.unwrap();
    // info!("resume to RAM content display");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xa4]).await.unwrap();
    // info!("set normal display");
    // i2c.write(ADDR, &[COMMAND_MODE, DISPLAY_NORMAL])
    //     .await
    //     .unwrap();
    // info!("set display clock divide ratio/oscillator frequency");
    // i2c.write(ADDR, &[COMMAND_MODE, 0xd5, 0x80]).await.unwrap();
    // info!("enable charge pump regulator");
    // i2c.write(ADDR, &[COMMAND_MODE, 0x8d, 0x14]).await.unwrap();

    // info!("clear display");
    // // Set memory addressing mode to horizontal
    // i2c.write(ADDR, &[COMMAND_MODE, 0x20, 0]).await.unwrap();
    // i2c.write(ADDR, &[COMMAND_MODE, 0x21, 0, 127])
    //     .await
    //     .unwrap();
    // i2c.write(ADDR, &[COMMAND_MODE, 0x22, 0, 7]).await.unwrap();
    // for page in 0..8 {
    //     i2c.write(ADDR, &send_blank_page).await.unwrap();
    // }

    // info!("turn on display");
    // i2c.write(ADDR, &[COMMAND_MODE, DISPLAY_ON]).await.unwrap();

    // info!("turn on ADC");
    // i2c.write(ADC_ADDR, &CONFIG_BYTES).await.unwrap();

    // let mut adc_val = [0u8; 2];

    // let mut x: i16 = 0;
    // let mut x_vel: i16 = 1;
    // let mut y: i16 = 0;
    // let mut y_vel: i16 = 1;

    // loop {
    //     x += x_vel;
    //     // If we hit the edge, reverse direction
    //     if x >= 127 || x <= 0 {
    //         x_vel *= -1;
    //         x = x.clamp(0, 127);
    //     }

    //     // // "Gravity"
    //     // y_vel += 1;

    //     // y += y_vel;
    //     // // If we hit the edge, reverse direction
    //     // if y >= 64 || y <= 0 {
    //     //     y_vel *= -1;
    //     //     if y_vel < 0 {
    //     //         // Remove some energy on bounce
    //     //         y_vel += 1;
    //     //     }
    //     //     y = y.clamp(0, 63);
    //     // }

    //     let mapped_y = if y < 16 { y } else { y - 1 };
    //     let page = mapped_y / 8;
    //     let bit = mapped_y % 8;
    //     let byte = if y == 16 { 0 } else { 1 << bit };

    //     i2c.write(ADDR, &[COMMAND_MODE, 0x21, x.try_into().unwrap(), 127])
    //         .await
    //         .unwrap();
    //     i2c.write(ADDR, &[COMMAND_MODE, 0x22, page.try_into().unwrap(), 7])
    //         .await
    //         .unwrap();
    //     for i in 0..3 {
    //         i2c.write(ADDR, &[DATA_MODE, byte]).await.unwrap();
    //     }

    //     Timer::after(Duration::from_millis(50)).await;

    //     i2c.write(ADC_ADDR, &[CONVERSION_REGISTER]).await.unwrap();
    //     i2c.read(ADC_ADDR, &mut adc_val).await.unwrap();

    //     let converted = i16::from_be_bytes(adc_val);
    //     info!("ADC: {:?}", converted);
    //     if (converted > 0) {
    //         y = (converted - 7000) / 200;
    //     }
    //     //info!("x: {}, y: {}", x, y);
    //     i2c.write(ADDR, &[COMMAND_MODE, 0x21, 0, 127])
    //         .await
    //         .unwrap();
    //     i2c.write(ADDR, &[COMMAND_MODE, 0x22, 0, 7]).await.unwrap();
    //     for page in 0..8 {
    //         i2c.write(ADDR, &send_blank_page).await.unwrap();
    //     }
    // }
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
