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

//! ST7796 display initialization for the Freenove board.

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
    // Declare the fixed 320-by-480 pixel controller geometry expected by this board.
    const FRAMEBUFFER_SIZE: (u16, u16) = (320, 480);
    /// Initializes the ST7796 controller with reset, wake, RGB565, power, gamma, and
    /// orientation commands.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut FreenoveSt7796`) - Receiver state used by this operation. Mutated in
    ///   place.
    /// * `di` (`&mut DI`) - Display interface used to send controller commands.
    /// * `delay` (`&mut DELAY`) - Delay provider used for controller reset and wake timing in
    ///   milliseconds.
    /// * `options` (`&ModelOptions`) - Display orientation and controller options supplied by
    ///   the mipidsi builder.
    ///
    /// # Type Parameters
    ///
    /// * `DELAY` - Delay implementation satisfying DelayNs for display startup timing.
    /// * `DI` - Controller interface implementation supplying command transport and its error
    ///   type.
    ///
    /// # Returns
    ///
    /// `Result<SetAddressMode, ModelInitError<DI::Error>>` - Ok with the selected address-mode
    /// command; Err with the display interface initialization error.
    ///
    /// # Errors
    ///
    /// Propagates errors from interface commands. The sequence retains TFT_eSPI attribution and
    /// licence conditions.
    fn init<DELAY: DelayNs, DI: Interface>(
        &mut self,
        di: &mut DI,
        delay: &mut DELAY,
        options: &ModelOptions,
    ) -> Result<SetAddressMode, ModelInitError<DI::Error>> {
        // Keep controller memory-address mode encoding the requested orientation and colour-order
        // options in this local variable for the following operations.
        let madctl = SetAddressMode::from(options);
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0x01, &[])?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0x11, &[])?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xf0, &[0xc3])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xf0, &[0x96])?;
        // Send the selected controller address mode so subsequent pixels use the requested
        // orientation.
        di.write_command(madctl)?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0x3a, &[0x55])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xb4, &[0x01])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xb6, &[0x80, 0x02, 0x3b])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xe8, &[0x40, 0x8a, 0x00, 0x00, 0x29, 0x19, 0xa5, 0x33])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xc1, &[0x06])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xc2, &[0xa7])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xc5, &[0x18])?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(
            0xe0,
            &[
                0xf0, 0x09, 0x0b, 0x06, 0x04, 0x15, 0x2f, 0x54, 0x42, 0x3c, 0x17, 0x14, 0x18, 0x1b,
            ],
        )?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(
            0xe1,
            &[
                0xe0, 0x09, 0x0b, 0x06, 0x04, 0x03, 0x2b, 0x43, 0x42, 0x3b, 0x16, 0x14, 0x17, 0x1b,
            ],
        )?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xf0, &[0x3c])?;
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0xf0, &[0x69])?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Send an ST7796 controller command with its associated parameter bytes.
        di.send_command(0x29, &[])?;
        // Wait the requested milliseconds so the display controller can complete its reset or wake
        // transition.
        delay.delay_ms(120);
        // Return success; the caller receives controller memory-address mode encoding the requested
        // orientation and colour-order options.
        Ok(madctl)
    }
}
