use defmt::warn;
use embassy_rp::{
    adc::{Adc, Async, Channel},
    gpio::{Input, Output},
    peripherals::{self},
    usb::Driver,
};
use embassy_time::Timer;
use embassy_usb::class::hid::{HidWriter, ReportId, RequestHandler};
use embassy_usb::control::OutResponse;

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

pub async fn handle_joystick(
    mut adc: Adc<'_, Async>,
    mut vx_analog: Channel<'_>,
    mut vy_analog: Channel<'_>,
    mut vz_analog: Channel<'_>,
    s1: Input<'_>,
    s2: Input<'_>,
    mut led_0: Output<'_>,
    mut led_1: Output<'_>,
    mut led_2: Output<'_>,
    mut led_3: Output<'_>,
    mut led_4: Output<'_>,
    mut led_5: Output<'_>,
    mut writer: HidWriter<'static, Driver<'static, peripherals::USB>, 8>,
) {
    let mut counter: u16 = 0;

    loop {
        _ = Timer::after_millis(1).await;
        let report = ControlPanelReport {
            x: -((adc.read(&mut vx_analog).await.unwrap_or_default() / 16) as i16 - 128) as i8,
            y: ((adc.read(&mut vy_analog).await.unwrap_or_default() / 16) as i16 - 128) as i8,
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

        // Update the LEDs.
        counter = counter.wrapping_add(1);
        if (counter & 0b00000100) != 0 {
            led_0.set_high();
        } else {
            led_0.set_low();
        }
        if (counter & 0b00001000) != 0 {
            led_1.set_high();
        } else {
            led_1.set_low();
        }
        if (counter & 0b00010000) != 0 {
            led_2.set_high();
        } else {
            led_2.set_low();
        }
        if (counter & 0b00100000) != 0 {
            led_3.set_high();
        } else {
            led_3.set_low();
        }
        if (counter & 0b01000000) != 0 {
            led_4.set_high();
        } else {
            led_4.set_low();
        }
        if (counter & 0b10000000) != 0 {
            led_5.set_high();
        } else {
            led_5.set_low();
        }
    }
}
