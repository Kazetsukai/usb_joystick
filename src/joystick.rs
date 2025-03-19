use defmt::warn;
use embassy_rp::{
    adc::{Adc, AdcPin, Async, Channel},
    gpio::{Input, Level, Output, Pin, Pull},
};
use embassy_time::Timer;
use embassy_usb::{
    class::hid::{self, HidReader, HidReaderWriter},
    control::OutResponse,
    Builder,
};
use embassy_usb::{
    class::hid::{HidWriter, ReportId, RequestHandler},
    driver::Driver,
};
use static_cell::StaticCell;
use usbd_hid::descriptor::SerializedDescriptor;

use crate::hid_descriptor::ControlPanelReport;

pub struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        defmt::info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        defmt::info!("Set report for {:?}: {=[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        defmt::info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        defmt::info!("Get idle rate for {:?}", id);
        None
    }
}

pub struct HidResponderRunner<'a, D>
where
    D: Driver<'a>,
{
    reader: HidReader<'a, D, 1>,
}
impl<'a, D: Driver<'a>> HidResponderRunner<'a, D> {
    pub async fn run(self) -> ! {
        self.reader.run(true, &mut MyRequestHandler {}).await;
    }
}

pub struct JoystickRunner<D>
where
    D: Driver<'static>,
{
    adc: Adc<'static, Async>,
    vx_analog: Channel<'static>,
    vy_analog: Channel<'static>,
    vz_analog: Channel<'static>,
    s1: Input<'static>,
    s2: Input<'static>,
    led_0: Output<'static>,
    led_1: Output<'static>,
    led_2: Output<'static>,
    led_3: Output<'static>,
    led_4: Output<'static>,
    led_5: Output<'static>,
    writer: HidWriter<'static, D, 8>,
}
impl<D: Driver<'static>> JoystickRunner<D> {
    pub async fn run(&mut self) -> ! {
        let mut counter: u16 = 0;

        loop {
            _ = Timer::after_millis(1).await;
            let report = ControlPanelReport {
                x: -((self.adc.read(&mut self.vx_analog).await.unwrap_or_default() / 16) as i16
                    - 128) as i8,
                y: ((self.adc.read(&mut self.vy_analog).await.unwrap_or_default() / 16) as i16
                    - 128) as i8,
                x2: -((self.adc.read(&mut self.vz_analog).await.unwrap_or_default() / 16) as i16
                    - 128) as i8,
                y2: 0,
                s1: if self.s1.is_low() { 0 } else { 255 },
                s2: if self.s2.is_low() { 0 } else { 255 },
            };
            // Send the report.
            match self.writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            }

            // Update the LEDs.
            counter = counter.wrapping_add(1);
            if (counter & 0b00000100) != 0 {
                self.led_0.set_high();
            } else {
                self.led_0.set_low();
            }
            if (counter & 0b00001000) != 0 {
                self.led_1.set_high();
            } else {
                self.led_1.set_low();
            }
            if (counter & 0b00010000) != 0 {
                self.led_2.set_high();
            } else {
                self.led_2.set_low();
            }
            if (counter & 0b00100000) != 0 {
                self.led_3.set_high();
            } else {
                self.led_3.set_low();
            }
            if (counter & 0b01000000) != 0 {
                self.led_4.set_high();
            } else {
                self.led_4.set_low();
            }
            if (counter & 0b10000000) != 0 {
                self.led_5.set_high();
            } else {
                self.led_5.set_low();
            }
        }
    }
}

pub(crate) fn make_joystick<D>(
    builder: &mut Builder<'static, D>,
    adc: Adc<'static, Async>,
    pin_vx: impl AdcPin,
    pin_vy: impl AdcPin,
    pin_vz: impl AdcPin,
    pin_s1: impl Pin,
    pin_s2: impl Pin,
    pin_led0: impl Pin,
    pin_led1: impl Pin,
    pin_led2: impl Pin,
    pin_led3: impl Pin,
    pin_led4: impl Pin,
    pin_led5: impl Pin,
) -> (JoystickRunner<D>, HidResponderRunner<'static, D>)
where
    D: Driver<'static>,
{
    let config = hid::Config {
        report_descriptor: ControlPanelReport::desc(),
        request_handler: None,
        poll_ms: 60,
        max_packet_size: 64,
    };
    let hid = {
        static STATE: StaticCell<hid::State> = StaticCell::new();
        let state = STATE.init(hid::State::new());
        HidReaderWriter::<_, 1, 8>::new(builder, state, config)
    };

    // Joystick setup
    let (reader, writer) = hid.split();

    let vx_analog = Channel::new_pin(pin_vx, Pull::None);
    let vy_analog = Channel::new_pin(pin_vy, Pull::None);
    let vz_analog = Channel::new_pin(pin_vz, Pull::None);
    let s1 = Input::new(pin_s1, Pull::Up);
    let s2 = Input::new(pin_s2, Pull::Up);

    let led_0 = Output::new(pin_led0, Level::Low);
    let led_1 = Output::new(pin_led1, Level::Low);
    let led_2 = Output::new(pin_led2, Level::Low);
    let led_3 = Output::new(pin_led3, Level::Low);
    let led_4 = Output::new(pin_led4, Level::Low);
    let led_5 = Output::new(pin_led5, Level::Low);

    let joystick = JoystickRunner {
        adc,
        vx_analog,
        vy_analog,
        vz_analog,
        s1,
        s2,
        led_0,
        led_1,
        led_2,
        led_3,
        led_4,
        led_5,
        writer,
    };

    let responder = HidResponderRunner { reader };

    (joystick, responder)
}
