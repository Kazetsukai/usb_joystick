use embassy_usb::{
    class::cdc_ncm::{
        self,
        embassy_net::{Device, Runner, State},
        CdcNcmClass,
    },
    driver::Driver,
    Builder,
};
use static_cell::StaticCell;

const MTU: usize = 1514;

pub(crate) fn make_usb_ethernet_device<D>(
    builder: &mut Builder<'static, D>,
) -> (Runner<'static, D, MTU>, Device<'static, MTU>)
where
    D: Driver<'static>,
{
    let our_mac_addr = [0xe2, 0x58, 0xb1, 0xe7, 0xfb, 0x12];
    let host_mac_addr = [0x82, 0x88, 0x88, 0x88, 0x88, 0x88];

    // Create classes on the builder.
    let cdc_ncm_class = {
        static STATE: StaticCell<cdc_ncm::State> = StaticCell::new();
        let state = STATE.init(cdc_ncm::State::new());
        CdcNcmClass::new(builder, state, host_mac_addr, 64)
    };

    static NET_STATE: StaticCell<State<MTU, 4, 4>> = StaticCell::new();
    let (runner, device) = cdc_ncm_class
        .into_embassy_net_device::<MTU, 4, 4>(NET_STATE.init(State::new()), our_mac_addr);

    (runner, device)
}
