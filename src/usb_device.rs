use embassy_usb::driver::Driver;
use embassy_usb::Builder;

use static_cell::StaticCell;

use crate::DEVICE_NAME;

pub fn get_usb_builder<D>(usb_driver: D) -> Builder<'static, D>
where
    D: Driver<'static>,
{
    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Kaze");
        config.product = Some(DEVICE_NAME);
        config.serial_number = Some("12345678");
        config.max_power = 500;
        config.max_packet_size_0 = 64;

        // Required for windows compatibility.
        config.composite_with_iads = true;
        config.device_class = 0xEF;
        config.device_sub_class = 0x02;
        config.device_protocol = 0x01;
        config
    };

    let builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            usb_driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [], // no msos descriptors
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    return builder;
}
