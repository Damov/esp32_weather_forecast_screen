// ============================================================================= //
// File          : display.rs                                                    //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// ST7796 display initialization for the Freenove board.                         //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Implements the mipidsi display model for the 320 by 480 ST7796 panel. Applies //
// orientation and RGB565 settings, reset and wake  delays, power and gamma com- //
// mands, and display activation. The initialization  is adapted from  TFT_eSPI; //
// its upstream licence notice remains applicable.                               //
//                                                                               //
// Note:                                                                         //
// -----                                                                         //
// ST7796 initialization matching Freenove's bundled TFT_eSPI 2.5.43 driver. Up- //
// stream copyright and licence: ../../assets/licenses/TFT_eSPI.txt              //
// (TFT_eSPI notice). Retain this notice with source  distributions and  binary  //
// releases containing this code.                                                //
// ============================================================================= //

use embedded_graphics::pixelcolor::Rgb565;
use embedded_hal::delay::DelayNs;
use mipidsi::{
    dcs::{InterfaceExt, SetAddressMode},
    interface::Interface,
    models::{Model, ModelInitError},
    options::ModelOptions,
};

pub struct FreenoveSt7796;
impl Model for FreenoveSt7796 {
    type ColorFormat = Rgb565;
    const FRAMEBUFFER_SIZE: (u16, u16) = (320, 480);
    fn init<DELAY: DelayNs, DI: Interface>(
        &mut self,
        di: &mut DI,
        delay: &mut DELAY,
        options: &ModelOptions,
    ) -> Result<SetAddressMode, ModelInitError<DI::Error>> {
        let madctl = SetAddressMode::from(options);
        delay.delay_ms(120);
        di.send_command(0x01, &[])?;
        delay.delay_ms(120);
        di.send_command(0x11, &[])?;
        delay.delay_ms(120);
        di.send_command(0xf0, &[0xc3])?;
        di.send_command(0xf0, &[0x96])?;
        di.write_command(madctl)?;
        di.send_command(0x3a, &[0x55])?;
        di.send_command(0xb4, &[0x01])?;
        di.send_command(0xb6, &[0x80, 0x02, 0x3b])?;
        di.send_command(0xe8, &[0x40, 0x8a, 0x00, 0x00, 0x29, 0x19, 0xa5, 0x33])?;
        di.send_command(0xc1, &[0x06])?;
        di.send_command(0xc2, &[0xa7])?;
        di.send_command(0xc5, &[0x18])?;
        delay.delay_ms(120);
        di.send_command(
            0xe0,
            &[
                0xf0, 0x09, 0x0b, 0x06, 0x04, 0x15, 0x2f, 0x54, 0x42, 0x3c, 0x17, 0x14, 0x18, 0x1b,
            ],
        )?;
        di.send_command(
            0xe1,
            &[
                0xe0, 0x09, 0x0b, 0x06, 0x04, 0x03, 0x2b, 0x43, 0x42, 0x3b, 0x16, 0x14, 0x17, 0x1b,
            ],
        )?;
        delay.delay_ms(120);
        di.send_command(0xf0, &[0x3c])?;
        di.send_command(0xf0, &[0x69])?;
        delay.delay_ms(120);
        di.send_command(0x29, &[])?;
        delay.delay_ms(120);
        Ok(madctl)
    }
}
