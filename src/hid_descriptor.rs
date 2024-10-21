use serde::ser::{Serialize, SerializeTuple, Serializer};
use usbd_hid::descriptor::gen_hid_descriptor;
use usbd_hid::descriptor::generator_prelude::*;

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = GAMEPAD) = {
      (collection = PHYSICAL, usage = POINTER) = {
        (usage = X,) = {
          #[item_settings data,variable] x=input;
        };
        (usage = Y,) = {
          #[item_settings data,variable] y=input;
        };
      };
      (collection = PHYSICAL, usage = POINTER) = {
        (usage = Z,) = {
          #[item_settings data,variable] x2=input;
        };
        (usage = 0x35,) = {
          #[item_settings data,variable] y2=input;
        };
      };
      (usage_page = BUTTON, usage_min = 1, usage_max = 2) = {
        #[item_settings data,variable] s1=input;
      };
    }
)]
pub struct ControlPanelReport {
    pub x: i8,
    pub y: i8,
    pub x2: i8,
    pub y2: i8,
    pub s1: u8,
    pub s2: u8,
}
