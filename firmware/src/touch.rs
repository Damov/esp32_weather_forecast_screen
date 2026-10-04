// ============================================================================= //
// File          : touch.rs                                                      //
// License       : GPL-3.0-only                                                  //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Resistive touch filtering, gestures, and calibration.                         //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Rejects noisy or invalid samples, debounces presses, and reports touch        //
// phases. Fits and validates coordinate calibration, serializes calibration     //
// data, and guides a timed calibration wizard with center verification. Commits //
// new mappings only after successful persistence.                               //
// ============================================================================= //

//! Hardware-independent resistive touch filtering, mapping, and calibration.
// Four calibration crosses in fixed portrait order: top-left, top-right, bottom-right, bottom-left.
// This fixed value is shared by the operations below.
pub const TARGETS: [(i32, i32); 4] = [(24, 24), (295, 24), (295, 455), (24, 455)];
// Screen-center verification cross at pixel coordinates (160, 240). This fixed value is shared by
// the operations below.
pub const CENTER: (i32, i32) = (160, 240);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawPoint {
    // Raw horizontal samples or affine coefficients used to map that axis. Stored as u16.
    pub x: u16,
    // Raw vertical samples or affine coefficients used to map that axis. Stored as u16.
    pub y: u16,
}
impl RawPoint {
    /// Checks whether both ADC coordinates lie strictly between the rails.
    ///
    /// # Arguments
    ///
    /// * `self` (`RawPoint`) - Receiver state used by this operation. Passed by value.
    ///
    /// # Returns
    ///
    /// `bool` - True for coordinates in 1-4094 on both axes; false otherwise.
    fn valid(self) -> bool {
        // Both ADC coordinates must lie in 1-4094; zero and 4095 are rejected as rail-valued
        // samples.
        self.x > 0 && self.x < 4095 && self.y > 0 && self.y < 4095
    }
}

/// Five conversions suppress ADC noise. Reject rail values, weak contact, and
/// samples that move too far during one measurement.
///
/// Filters five touch conversions using median coordinates and pressure.
///
/// # Arguments
///
/// * `samples` (`[(RawPoint, u16); 5]`) - Five pairs of 12-bit raw coordinates and pressure
///   readings; median pressure must be at least 600.
///
/// # Returns
///
/// `Option<RawPoint>` - Some median point when contact pressure is at least 600 and
/// coordinate spread is at most 150; None for invalid or noisy samples.
pub fn filter(samples: [(RawPoint, u16); 5]) -> Option<RawPoint> {
    // Keep raw horizontal samples or affine coefficients used to map that axis in this local
    // variable for the following operations.
    let mut x = [0; 5];
    // Keep raw vertical samples or affine coefficients used to map that axis in this local variable
    // for the following operations.
    let mut y = [0; 5];
    // Keep five contact-strength samples; their median must be at least 600 in this local variable
    // for the following operations.
    let mut pressure = [0; 5];
    // Copy each of the five validated coordinate/pressure samples into its per-axis median-filter
    // array.
    for (i, (point, z)) in samples.into_iter().enumerate() {
        // Reject any sample touching a 12-bit ADC rail, which commonly indicates invalid or
        // disconnected touch input.
        if !point.valid() {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Store the validated sample’s raw horizontal coordinate for the median calculation.
        x[i] = point.x;
        // Store the validated sample’s raw vertical coordinate for the median calculation.
        y[i] = point.y;
        // Store the sample’s contact strength so median pressure can reject weak touches.
        pressure[i] = z;
    }
    // Sort the small sample array so its middle value can be used as a noise-resistant median.
    x.sort_unstable();
    // Sort the small sample array so its middle value can be used as a noise-resistant median.
    y.sort_unstable();
    // Sort the small sample array so its middle value can be used as a noise-resistant median.
    pressure.sort_unstable();
    // Reject weak median pressure or movement exceeding 150 ADC counts during one measurement; only
    // stable contact is accepted.
    if pressure[2] < 600 || x[4] - x[0] > 150 || y[4] - y[0] > 150 {
        // Leave this function now with no available value; later statements are skipped.
        return None;
    }
    // Initialize raw horizontal samples or affine coefficients used to map that axis from the
    // supplied value.
    Some(RawPoint { x: x[2], y: y[2] })
}

/// Require two contact readings and three release readings, avoiding repeated
/// actions when a held finger briefly produces a weak sample.
#[derive(Default)]
pub struct Press {
    // Whether the current gesture has already produced its Down action. Stored as bool.
    held: bool,
    // Consecutive present samples; two confirm a new touch. Stored as u8.
    contact: u8,
    // Consecutive absent samples; three release the held gesture. Stored as u8.
    release: u8,
}
impl Press {
    /// Debounces contact and release while suppressing repeated press actions.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Press`) - Debounced contact and release counters for the current
    ///   gesture. Mutated in place.
    /// * `point` (`Option<RawPoint>`) - Raw touch point; None represents absent or rejected
    ///   contact when optional.
    ///
    /// # Returns
    ///
    /// `Option<RawPoint>` - Some point after two contact samples start a press; None while
    /// waiting, held, or releasing. Three absent samples release the hold.
    pub fn update(&mut self, point: Option<RawPoint>) -> Option<RawPoint> {
        // A valid contact sample resets release counting and contributes toward a confirmed new
        // press.
        if let Some(point) = point {
            // Set consecutive absent samples to 0.
            self.release = 0;
            // Set consecutive present samples to the advanced value clamped at the integer limit
            // instead of overflow.
            self.contact = self.contact.saturating_add(1);
            // Two consecutive contacts confirm one new press; held gestures must not produce
            // repeated Down actions.
            if !self.held && self.contact >= 2 {
                // Set whether the current gesture has already produced its Down action to the
                // enabled state.
                self.held = true;
                // Leave this function now with an available value for coordinate sample associated
                // with the current touch or pixel operation; later statements are skipped.
                return Some(point);
            }
        } else {
            // Set consecutive present samples to 0.
            self.contact = 0;
            // Set consecutive absent samples to the advanced value clamped at the integer limit
            // instead of overflow.
            self.release = self.release.saturating_add(1);
            // Three absent samples confirm release; a single missing sample must not split a held
            // gesture.
            if self.release >= 3 {
                // Set whether the current gesture has already produced its Down action to the
                // disabled state.
                self.held = false;
            }
        }
        // Return no available value as the value of this block.
        None
    }
}

#[derive(Clone, Copy)]
pub struct Calibration {
    // Four raw corner samples retained for validation and the 17-byte persistence record. Stored as
    // [RawPoint; 4].
    points: [RawPoint; 4],
    // Raw horizontal samples or affine coefficients used to map that axis. Stored as [i64; 3].
    x: [i64; 3],
    // Raw vertical samples or affine coefficients used to map that axis. Stored as [i64; 3].
    y: [i64; 3],
    // Signed triangle determinant; small magnitude rejects poorly separated calibration points.
    // Stored as i64.
    divisor: i64,
}
impl Calibration {
    /// Fits a portrait coordinate transform and validates its fourth corner.
    ///
    /// # Arguments
    ///
    /// * `points` (`[RawPoint; 4]`) - Four raw corner samples in TARGETS order: top-left,
    ///   top-right, bottom-right, bottom-left.
    ///
    /// # Returns
    ///
    /// `Option<Self>` - Some mapping for valid, sufficiently separated samples and a fourth
    /// corner within 20 pixels; None otherwise.
    pub fn fit(points: [RawPoint; 4]) -> Option<Self> {
        // Reject corner sets containing rail-valued or otherwise invalid raw coordinates before
        // fitting a transform.
        if !points.iter().all(|p| p.valid()) {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Keep [a, b, c,  ] in this local variable for the following operations.
        let [a, b, c, _] = points;
        // Keep the returned components: `ax` holds ax, `ay` holds ay, `bx` holds bx, `by` holds by,
        // `cx` holds cx, `cy` holds cy in this local variable for the following operations.
        let (ax, ay, bx, by, cx, cy) = (
            a.x as i64, a.y as i64, b.x as i64, b.y as i64, c.x as i64, c.y as i64,
        );
        // Compute the signed triangle determinant; its magnitude measures whether the raw corners
        // are sufficiently separated.
        let divisor = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
        // The first three samples are too close to collinear to produce a reliable affine mapping.
        if divisor.abs() < 200_000 {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Keep closure computing each affine axis from three raw corners and known screen targets
        // in this local variable for the following operations.
        let coefficients = |values: [i64; 3]| {
            // Calculate the raw-x coefficient that maps the calibration triangle onto this screen
            // axis.
            let u = (values[1] - values[0]) * (cy - ay) - (values[2] - values[0]) * (by - ay);
            // Calculate the raw-y coefficient for the same affine axis, allowing swapped or
            // inverted touch wiring.
            let v = (bx - ax) * (values[2] - values[0]) - (cx - ax) * (values[1] - values[0]);
            // Return the ordered sample/byte array as the value of this block.
            [u, v, values[0] * divisor - u * ax - v * ay]
        };
        // Keep validated touch-coordinate transform in this local variable for the following
        // operations.
        let cal = Self {
            // Initialize four raw corner samples retained for validation and the 17-byte
            // persistence record from the supplied value.
            points,
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            x: coefficients([24, 295, 295]),
            // Initialize raw vertical samples or affine coefficients used to map that axis from the
            // supplied value.
            y: coefficients([24, 24, 455]),
            // Initialize signed triangle determinant; small magnitude rejects poorly separated
            // calibration points from the supplied value.
            divisor,
        };
        // Project the unused fourth corner to independently check that the transform agrees with
        // the physical panel.
        let fourth = cal.project(points[3]);
        // Reject a fit whose independently projected fourth corner differs by more than 20 pixels
        // on either axis.
        if (fourth.0 - TARGETS[3].0).abs() > 20 || (fourth.1 - TARGETS[3].1).abs() > 20 {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Return an available validated touch-coordinate transform.
        Some(cal)
    }
    /// Projects raw coordinates through the fitted affine transform.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Calibration`) - Validated affine mapping and original raw corner samples.
    ///   Borrowed without changing it.
    /// * `p` (`RawPoint`) - Raw 12-bit touch coordinates to transform or validate.
    ///
    /// # Returns
    ///
    /// `(i32, i32)` - Unclamped screen x and y coordinates in pixels.
    fn project(&self, p: RawPoint) -> (i32, i32) {
        // Keep closure evaluating one affine axis with integer arithmetic and the shared
        // determinant in this local variable for the following operations.
        let axis =
            |c: [i64; 3]| ((c[0] * p.x as i64 + c[1] * p.y as i64 + c[2]) / self.divisor) as i32;
        // Return the ordered tuple of related values as the value of this block.
        (axis(self.x), axis(self.y))
    }
    /// Validates raw coordinates and maps them into the portrait display.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Calibration`) - Validated affine mapping and original raw corner samples.
    ///   Borrowed without changing it.
    /// * `p` (`RawPoint`) - Raw 12-bit touch coordinates to transform or validate.
    ///
    /// # Returns
    ///
    /// `Option<(i32, i32)>` - Some coordinates clamped to 0-319 and 0-479 for points within the
    /// tolerated border; None for invalid or distant points.
    pub fn map(&self, p: RawPoint) -> Option<(i32, i32)> {
        // Reject invalid raw coordinates before applying the stored calibration.
        if !p.valid() {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Keep the returned components: `x` holds raw horizontal samples or affine coefficients
        // used to map that axis, `y` holds raw vertical samples or affine coefficients used to map
        // that axis in this local variable for the following operations.
        let (x, y) = self.project(p);
        // Reject touches beyond the tolerated ten-pixel border; nearby edge touches can be safely
        // clamped afterward.
        if !(-10..330).contains(&x) || !(-10..490).contains(&y) {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Return an available the ordered tuple of related values.
        Some((x.clamp(0, 319), y.clamp(0, 479)))
    }
    /// Checks whether a sample maps within 20 pixels of the center target.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Calibration`) - Validated affine mapping and original raw corner samples.
    ///   Borrowed without changing it.
    /// * `p` (`RawPoint`) - Raw 12-bit touch coordinates to transform or validate.
    ///
    /// # Returns
    ///
    /// `bool` - True for a valid point near the center on both axes; false otherwise.
    pub fn verify_center(&self, p: RawPoint) -> bool {
        // Execute is some and for the transformed value or entries produced by the closure.
        self.map(p)
            .is_some_and(|(x, y)| (x - CENTER.0).abs() <= 20 && (y - CENTER.1).abs() <= 20)
    }
    /// Version 1 is bound to this board's fixed portrait orientation.
    ///
    /// Serializes the calibration corner samples for the fixed portrait board.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Calibration`) - Validated affine mapping and original raw corner samples.
    ///   Borrowed without changing it.
    ///
    /// # Returns
    ///
    /// `[u8; 17]` - 17 bytes containing version 1 and four little-endian coordinate pairs.
    pub fn encode(&self) -> [u8; 17] {
        // Keep version byte followed by four little-endian x/y pairs in this local variable for the
        // following operations.
        let mut bytes = [0; 17];
        // Set the selected entry from version byte followed by four little-endian x/y pairs to 1.
        bytes[0] = 1;
        // Visit each entry in zip result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for (point, out) in self.points.iter().zip(bytes[1..].chunks_exact_mut(4)) {
            // Copy the supplied bytes into an equally sized destination slice.
            out[..2].copy_from_slice(&point.x.to_le_bytes());
            // Copy the supplied bytes into an equally sized destination slice.
            out[2..].copy_from_slice(&point.y.to_le_bytes());
        }
        // Return version byte followed by four little-endian x/y pairs as the value of this block.
        bytes
    }
    /// Restores and revalidates a version-1 calibration record.
    ///
    /// # Arguments
    ///
    /// * `bytes` (`&[u8]`) - Version-1 calibration record; must contain exactly 17 bytes.
    ///
    /// # Returns
    ///
    /// `Option<Self>` - Some fitted mapping for valid 17-byte data; None for wrong length,
    /// version, or corner geometry.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        // Only the exact version-1 record shape is supported: one version byte and four raw
        // coordinate pairs.
        if bytes.len() != 17 || bytes[0] != 1 {
            // Leave this function now with no available value; later statements are skipped.
            return None;
        }
        // Keep four raw corner samples retained for validation and the 17-byte persistence record
        // in this local variable for the following operations.
        let mut points = [RawPoint { x: 0, y: 0 }; 4];
        // Visit each entry in zip result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for (point, chunk) in points.iter_mut().zip(bytes[1..].chunks_exact(4)) {
            // Set raw horizontal samples or affine coefficients used to map that axis to the
            // integer decoded from little-endian bytes.
            point.x = u16::from_le_bytes([chunk[0], chunk[1]]);
            // Set raw vertical samples or affine coefficients used to map that axis to the integer
            // decoded from little-endian bytes.
            point.y = u16::from_le_bytes([chunk[2], chunk[3]]);
        }
        // Fits a portrait coordinate transform and validates its fourth corner.
        Self::fit(points)
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for touch behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Provides a stable set of raw calibration corners.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `[RawPoint; 4]` - Four raw points in the calibration target order.
    fn corners() -> [RawPoint; 4] {
        // Return the ordered sample/byte array as the value of this block.
        [
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 400, y: 500 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 500 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 3600 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 400, y: 3600 },
        ]
    }
    /// Verifies calibrated corner and center positions and equivalent mappings after serialized
    /// restoration.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn maps_corners_center_and_restarts() {
        // Keep validated touch-coordinate transform in this local variable for the following
        // operations.
        let cal = Calibration::fit(corners()).unwrap();
        // Visit each entry in zip result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for (p, target) in corners().into_iter().zip(TARGETS) {
            // Verify that the transformed value or entries produced by the closure exactly matches
            // an available value for requested screen state or calibration position, selected
            // before applying its side effects.
            assert_eq!(cal.map(p), Some(target));
        }
        // Verify checks whether a sample maps within 20 pixels of the center target. A violation
        // means the tested behavior is incorrect.
        assert!(cal.verify_center(RawPoint { x: 1955, y: 2050 }));
        // Verify the inverse of checks whether a sample maps within 20 pixels of the center target.
        // A violation means the tested behavior is incorrect.
        assert!(!cal.verify_center(corners()[0]));
        // Keep value decoded after persistence or recreated from the prior saved profile in this
        // local variable for the following operations.
        let restored = Calibration::decode(&cal.encode()).unwrap();
        // Verify that the transformed value or entries produced by the closure exactly matches an
        // available value for the selected entry from four calibration crosses in fixed portrait
        // order: top-left, top-right, bottom-right, bottom-left.
        assert_eq!(restored.map(corners()[2]), Some(TARGETS[2]));
    }
    /// Verifies affine calibration for swapped or inverted raw coordinate axes.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn swapped_and_inverted_axes_map_correctly() {
        // Keep four raw corner samples retained for validation and the 17-byte persistence record
        // in this local variable for the following operations.
        let points = corners().map(|p| RawPoint {
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            x: 4095 - p.y,
            // Initialize raw vertical samples or affine coefficients used to map that axis from the
            // supplied value.
            y: p.x,
        });
        // Keep validated touch-coordinate transform in this local variable for the following
        // operations.
        let cal = Calibration::fit(points).unwrap();
        // Visit each entry in zip result for the surrounding operation; the loop binding provides
        // its value or index for this iteration.
        for (p, target) in points.into_iter().zip(TARGETS) {
            // Verify that the transformed value or entries produced by the closure exactly matches
            // an available value for requested screen state or calibration position, selected
            // before applying its side effects.
            assert_eq!(cal.map(p), Some(target));
        }
    }
    /// Verifies rejection of degenerate calibration records and raw touches outside the
    /// tolerated display border.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn invalid_calibration_and_outside_touches_are_rejected() {
        // Verify fits a portrait coordinate transform and validates its fourth corner is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(Calibration::fit([corners()[0]; 4]).is_none());
        // Keep four raw corner samples retained for validation and the 17-byte persistence record
        // in this local variable for the following operations.
        let mut points = corners();
        // Set the selected entry from four raw corner samples retained for validation and the
        // 17-byte persistence record to the selected entry from four raw corner samples retained
        // for validation and the 17-byte persistence record.
        points[3] = points[2];
        // Verify fits a portrait coordinate transform and validates its fourth corner is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(Calibration::fit(points).is_none());
        // Verify restores and revalidates a version-1 calibration record is unavailable. A
        // violation means the tested behavior is incorrect.
        assert!(Calibration::decode(&[0; 17]).is_none());
        // Keep validated touch-coordinate transform in this local variable for the following
        // operations.
        let cal = Calibration::fit(corners()).unwrap();
        // Verify restores and revalidates a version-1 calibration record is unavailable. A
        // violation means the tested behavior is incorrect.
        assert!(Calibration::decode(&cal.encode()[..16]).is_none());
        // Verify the transformed value or entries produced by the closure is unavailable. A
        // violation means the tested behavior is incorrect.
        assert!(cal.map(RawPoint { x: 1, y: 1 }).is_none());
        // Verify the transformed value or entries produced by the closure is unavailable. A
        // violation means the tested behavior is incorrect.
        assert!(cal.map(RawPoint { x: 0, y: 500 }).is_none());
    }
    /// Verifies median touch filtering and rejection of noisy, weak-pressure, or rail-valued
    /// samples.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn noise_pressure_and_disconnected_adc_are_rejected() {
        // Keep short-lived borrow or raw point used by the surrounding preference/touch operation
        // in this local variable for the following operations.
        let p = RawPoint { x: 1000, y: 2000 };
        // Verify that filters five touch conversions using median coordinates and pressure exactly
        // matches an available value for short-lived borrow or raw point used by the surrounding
        // preference/touch operation.
        assert_eq!(filter([(p, 800); 5]), Some(p));
        // Verify filters five touch conversions using median coordinates and pressure is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(filter([(p, 100); 5]).is_none());
        // Verify filters five touch conversions using median coordinates and pressure is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(filter([(RawPoint { x: 0, y: 0 }, 4095); 5]).is_none());
        // Keep five raw coordinate/pressure conversions used for stable touch filtering in this
        // local variable for the following operations.
        let mut samples = [(p, 800); 5];
        // Set raw horizontal samples or affine coefficients used to map that axis to 1200.
        samples[0].0.x = 1200;
        // Verify filters five touch conversions using median coordinates and pressure is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(filter(samples).is_none());
    }
    /// Verifies one action per held gesture and tolerance of brief missing contact samples.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn one_action_per_press_survives_brief_contact_loss() {
        // Keep short-lived borrow or raw point used by the surrounding preference/touch operation
        // in this local variable for the following operations.
        let p = RawPoint { x: 1000, y: 2000 };
        // Keep touch debouncer that suppresses repeated actions during a held gesture in this local
        // variable for the following operations.
        let mut press = Press::default();
        // Verify debounces contact and release while suppressing repeated press actions is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(press.update(Some(p)).is_none());
        // Verify that debounces contact and release while suppressing repeated press actions
        // exactly matches an available value for short-lived borrow or raw point used by the
        // surrounding preference/touch operation.
        assert_eq!(press.update(Some(p)), Some(p));
        // Verify debounces contact and release while suppressing repeated press actions is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(press.update(None).is_none());
        // Verify debounces contact and release while suppressing repeated press actions is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(press.update(Some(p)).is_none());
        // Visit each entry in 0..3; the loop binding provides its value or index for this
        // iteration.
        for _ in 0..3 {
            // Verify debounces contact and release while suppressing repeated press actions is
            // unavailable. A violation means the tested behavior is incorrect.
            assert!(press.update(None).is_none());
        }
        // Verify debounces contact and release while suppressing repeated press actions is
        // unavailable. A violation means the tested behavior is incorrect.
        assert!(press.update(Some(p)).is_none());
        // Verify that debounces contact and release while suppressing repeated press actions
        // exactly matches an available value for short-lived borrow or raw point used by the
        // surrounding preference/touch operation.
        assert_eq!(press.update(Some(p)), Some(p));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    // First debounced contact point of a new touch gesture.
    Down(RawPoint),
    // Updated coordinates while the accepted gesture remains held.
    Move(RawPoint),
    // Debounced release event ending the current gesture.
    Up,
}
impl Press {
    /// Creates a held press state that suppresses actions until release.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `Self` - Debouncer initialized to consume the existing gesture.
    pub fn until_release() -> Self {
        Self {
            // Initialize whether the current gesture has already produced its Down action as
            // enabled/active.
            held: true,
            // Start consecutive present samples; two confirm a new touch at zero; later operations
            // update it as needed.
            contact: 0,
            // Start consecutive absent samples; three release the held gesture at zero; later
            // operations update it as needed.
            release: 0,
        }
    }
    /// Converts debounced contact into press, movement, and release events.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Press`) - Debounced contact and release counters for the current
    ///   gesture. Mutated in place.
    /// * `point` (`Option<RawPoint>`) - Raw touch point; None represents absent or rejected
    ///   contact when optional.
    ///
    /// # Returns
    ///
    /// `Option<TouchPhase>` - Some Down, Move, or Up phase when available; None while contact
    /// is unsettled or absent.
    pub fn event(&mut self, point: Option<RawPoint>) -> Option<TouchPhase> {
        // Keep whether the current gesture has already produced its Down action in this local
        // variable for the following operations.
        let held = self.held;
        // A newly debounced contact starts the gesture and produces its Down event.
        if let Some(point) = self.update(point) {
            // Leave this function now with an available value for Down result for the surrounding
            // operation; later statements are skipped.
            return Some(TouchPhase::Down(point));
        }
        // The gesture has just transitioned from held to released, so emit exactly one Up event.
        if held && !self.held {
            // Leave this function now with an available value for TouchPhase Up; later statements
            // are skipped.
            return Some(TouchPhase::Up);
        }
        // While held, accepted coordinates become Move events instead of new button presses.
        if self.held {
            // Leave this function now with the transformed value or entries produced by the
            // closure; later statements are skipped.
            return point.map(TouchPhase::Move);
        }
        // Return no available value as the value of this block.
        None
    }
}

pub struct Wizard {
    // Current calibration target index; four corners are followed by center verification. Stored as
    // usize.
    pub stage: usize,
    // Four raw corner samples retained for validation and the 17-byte persistence record. Stored as
    // [RawPoint; 4].
    points: [RawPoint; 4],
    // Proposed value that has not yet replaced the active or persisted baseline. Stored as
    // Option<Calibration>.
    candidate: Option<Calibration>,
    // Monotonic time at which the pending operation expires. Stored as u64.
    deadline: u64,
    // Touch debouncer that suppresses repeated actions during a held gesture. Stored as Press.
    press: Press,
}
pub enum WizardResult {
    // No complete debounced calibration action is available yet.
    Waiting,
    // A corner was accepted and the next target should be shown.
    Advanced,
    // A fitted map passed center verification and may be persisted.
    Verified(Calibration),
    // Corner geometry or center verification rejected the proposed mapping.
    Invalid,
    // No required calibration action arrived before the per-stage deadline.
    TimedOut,
    // The user cancelled calibration before activating a replacement mapping.
    Cancelled,
}
impl Wizard {
    /// Starts a calibration session waiting for the current touch to release.
    ///
    /// # Arguments
    ///
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `Self` - Wizard at the first corner with a 60-second deadline.
    pub fn new(now: u64) -> Self {
        Self {
            // Start current calibration target index; four corners are followed by center
            // verification at zero; later operations update it as needed.
            stage: 0,
            // Initialize four raw corner samples retained for validation and the 17-byte
            // persistence record from the supplied value.
            points: [RawPoint { x: 0, y: 0 }; 4],
            // Leave proposed value that has not yet replaced the active or persisted baseline
            // unavailable until a later operation supplies it.
            candidate: None,
            // Initialize monotonic time at which the pending operation expires from the supplied
            // value.
            deadline: now + 60_000,
            // Initialize touch debouncer that suppresses repeated actions during a held gesture
            // from the supplied value.
            press: Press::until_release(),
        }
    }
    /// Returns the screen target for the current calibration stage.
    ///
    /// # Arguments
    ///
    /// * `self` (`&Wizard`) - Calibration stage, collected samples, and per-stage deadline.
    ///   Borrowed without changing it.
    ///
    /// # Returns
    ///
    /// `(i32, i32)` - Corner target for stages 0-3, or the center verification target
    /// afterward, in pixels.
    pub fn target(&self) -> (i32, i32) {
        // The first four stages collect corner samples; later stages verify the fitted mapping at
        // the center.
        if self.stage < 4 {
            TARGETS[self.stage]
        } else {
            // Return screen-center verification cross at pixel coordinates (160, 240) as the value
            // of this block.
            CENTER
        }
    }
    /// Processes cancellation, timeout, and debounced calibration samples.
    ///
    /// # Arguments
    ///
    /// * `self` (`&mut Wizard`) - Calibration stage, collected samples, and per-stage deadline.
    ///   Mutated in place.
    /// * `point` (`Option<RawPoint>`) - Raw touch point; None represents absent or rejected
    ///   contact when optional.
    /// * `cancel` (`bool`) - Whether the calibration session should be cancelled immediately.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `WizardResult` - Waiting, Advanced, Verified, Invalid, TimedOut, or Cancelled according
    /// to the session state; verified results contain the candidate mapping.
    pub fn update(&mut self, point: Option<RawPoint>, cancel: bool, now: u64) -> WizardResult {
        // Honor the cancellation request immediately, without persisting or activating an
        // incomplete mapping.
        if cancel {
            // Leave this function now with Cancelled state; later statements are skipped.
            return WizardResult::Cancelled;
        }
        // The sixty-second per-target deadline expired; require a fresh calibration attempt.
        if now >= self.deadline {
            // Leave this function now with TimedOut state; later statements are skipped.
            return WizardResult::TimedOut;
        }
        // Keep Some(point) in this local variable for the following operations.
        let Some(point) = self.press.update(point) else {
            // Leave this function now with Waiting state; later statements are skipped.
            return WizardResult::Waiting;
        };
        // The first four stages collect corner samples; later stages verify the fitted mapping at
        // the center.
        if self.stage < 4 {
            // Set the selected entry from four raw corner samples retained for validation and the
            // 17-byte persistence record to coordinate sample associated with the current touch or
            // pixel operation.
            self.points[self.stage] = point;
            // Update current calibration target index using 1, retaining the accumulated state for
            // subsequent steps.
            self.stage += 1;
            // Set monotonic time at which the pending operation expires to monotonic milliseconds
            // used for the wizard's sixty-second per-target deadline plus 60_000.
            self.deadline = now + 60_000;
            // All four corners are available, so fit and validate a candidate before requesting
            // center verification.
            if self.stage == 4 {
                // Set proposed value that has not yet replaced the active or persisted baseline to
                // fits a portrait coordinate transform and validates its fourth corner.
                self.candidate = Calibration::fit(self.points);
                // Corner geometry failed validation; stop rather than asking the user to verify an
                // unusable mapping.
                if self.candidate.is_none() {
                    // Leave this function now with Invalid state; later statements are skipped.
                    return WizardResult::Invalid;
                }
            }
            WizardResult::Advanced
        } else {
            // Choose the appropriate path for filters five touch conversions using median
            // coordinates and pressure; each arm handles one supported case.
            match self.candidate.filter(|cal| cal.verify_center(point)) {
                // Handle the Some(cal) case: Run the Verified operation with the supplied inputs.
                Some(cal) => WizardResult::Verified(cal),
                // Handle the None case: apply the state-specific behavior shown here.
                None => WizardResult::Invalid,
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Tests for calibration gesture sequencing, cancellation, and validation.
#[cfg(test)]
mod wizard_tests {
    use super::*;
    /// Feeds absent touch samples until a calibration-test gesture is released.
    ///
    /// # Arguments
    ///
    /// * `wizard` (`&mut Wizard`) - Mutable calibration wizard retaining stage, samples, and
    ///   deadline.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `()` - No value; advances the wizard's debounce state using three absent samples.
    fn release(wizard: &mut Wizard, now: u64) {
        // Visit each entry in 0..3; the loop binding provides its value or index for this
        // iteration.
        for _ in 0..3 {
            // Debounces contact and release while suppressing repeated press actions.
            wizard.update(None, false, now);
        }
    }
    /// Feeds contact samples to produce a debounced calibration-test press.
    ///
    /// # Arguments
    ///
    /// * `wizard` (`&mut Wizard`) - Mutable calibration wizard retaining stage, samples, and
    ///   deadline.
    /// * `point` (`RawPoint`) - Raw touch point; None represents absent or rejected contact
    ///   when optional.
    /// * `now` (`u64`) - Monotonic timestamp in milliseconds, using the same origin as stored
    ///   deadlines.
    ///
    /// # Returns
    ///
    /// `WizardResult` - Wizard result after the synthetic press has been processed.
    fn press(wizard: &mut Wizard, point: RawPoint, now: u64) -> WizardResult {
        // Debounces contact and release while suppressing repeated press actions.
        wizard.update(Some(point), false, now);
        // Debounces contact and release while suppressing repeated press actions.
        wizard.update(Some(point), false, now)
    }
    /// Verifies release gating, corner collection, and mandatory center verification before
    /// producing a candidate.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn wizard_waits_for_release_and_verifies_before_saving() {
        // Keep interactive calibration state, including collected corners and a timeout in this
        // local variable for the following operations.
        let mut wizard = Wizard::new(0);
        // Keep coordinate sample associated with the current touch or pixel operation in this local
        // variable for the following operations.
        let point = RawPoint { x: 400, y: 500 };
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            press(&mut wizard, point, 10),
            WizardResult::Waiting
        ));
        // Run each listed scenario or target in the declared order; the loop variables select the
        // data for that case.
        for point in [
            point,
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 500 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 3600 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 400, y: 3600 },
        ] {
            // Feeds absent touch samples until a calibration-test gesture is released.
            release(&mut wizard, 20);
            // Verify the formatted text or fixture created by matches. A violation means the tested
            // behavior is incorrect.
            assert!(matches!(
                press(&mut wizard, point, 30),
                WizardResult::Advanced
            ));
            // Verify the formatted text or fixture created by matches. A violation means the tested
            // behavior is incorrect.
            assert!(matches!(
                press(&mut wizard, point, 40),
                WizardResult::Waiting
            ));
        }
        // Verify that returns the screen target for the current calibration stage exactly matches
        // screen-center verification cross at pixel coordinates (160, 240).
        assert_eq!(wizard.target(), CENTER);
        // Feeds absent touch samples until a calibration-test gesture is released.
        release(&mut wizard, 50);
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            press(&mut wizard, RawPoint { x: 1955, y: 2050 }, 60),
            WizardResult::Verified(_)
        ));
    }
    /// Verifies that cancellation, expiry, and invalid calibration geometry cannot produce a
    /// verified mapping.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn cancel_timeout_and_bad_geometry_never_produce_a_candidate() {
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            Wizard::new(0).update(None, true, 10),
            WizardResult::Cancelled
        ));
        // Verify the formatted text or fixture created by matches. A violation means the tested
        // behavior is incorrect.
        assert!(matches!(
            Wizard::new(0).update(None, false, 60_000),
            WizardResult::TimedOut
        ));
        // Keep interactive calibration state, including collected corners and a timeout in this
        // local variable for the following operations.
        let mut wizard = Wizard::new(0);
        // Visit each entry in 0..4; the loop binding provides its value or index for this
        // iteration.
        for i in 0..4 {
            // Feeds absent touch samples until a calibration-test gesture is released.
            release(&mut wizard, 10);
            // Keep success or failure produced by the operation, retained for later handling in
            // this local variable for the following operations.
            let result = press(&mut wizard, RawPoint { x: 400, y: 500 }, 20);
            // Check whether i equals 3.
            if i == 3 {
                // Verify the formatted text or fixture created by matches. A violation means the
                // tested behavior is incorrect.
                assert!(matches!(result, WizardResult::Invalid));
            }
        }
    }
    /// Verifies debounced Down events together with subsequent movement and release phases.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn touch_phases_keep_button_debounce_and_report_drag_and_release() {
        // Keep touch debouncer that suppresses repeated actions during a held gesture in this local
        // variable for the following operations.
        let mut press = Press::default();
        // Keep short-lived borrow or raw point used by the surrounding preference/touch operation
        // in this local variable for the following operations.
        let p = RawPoint { x: 1000, y: 1000 };
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches no available value.
        assert_eq!(press.event(Some(p)), None);
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches an available value for Down result for the surrounding operation.
        assert_eq!(press.event(Some(p)), Some(TouchPhase::Down(p)));
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches an available value for Move result for the surrounding operation.
        assert_eq!(press.event(Some(p)), Some(TouchPhase::Move(p)));
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches no available value.
        assert_eq!(press.event(None), None);
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches no available value.
        assert_eq!(press.event(None), None);
        // Verify that converts debounced contact into press, movement, and release events exactly
        // matches an available value for TouchPhase Up.
        assert_eq!(press.event(None), Some(TouchPhase::Up));
    }
}

/// The active map changes only after the persistence callback succeeds.
///
/// Persists a candidate calibration before replacing the active mapping.
///
/// # Arguments
///
/// * `active` (`&mut Calibration`) - Active coordinate mapping; replaced only after
///   successful persistence.
/// * `candidate` (`Calibration`) - Verified replacement mapping to persist before
///   activation.
/// * `write` (`impl FnOnce(&[u8; 17]) -> Result<(), E>`) - One-shot persistence callback
///   receiving the 17-byte candidate encoding; returns Ok(()) or Err(E) without activating
///   the map itself.
///
/// # Type Parameters
///
/// * `E` - Caller-defined error returned by the hardware or persistence callback.
///
/// # Returns
///
/// `Result<(), E>` - Ok after persistence and replacement, or Err from the writer with the
/// existing map preserved.
///
/// # Errors
///
/// Propagates the persistence callback's error without changing the active calibration.
pub fn commit_calibration<E>(
    active: &mut Calibration,
    candidate: Calibration,
    write: impl FnOnce(&[u8; 17]) -> Result<(), E>,
) -> Result<(), E> {
    // Run the write operation with the supplied inputs.
    write(&candidate.encode())?;
    // Set currently used calibration mapping, replaced only after a successful save to proposed
    // value that has not yet replaced the active or persisted baseline.
    *active = candidate;
    // Return success after the required side effects are complete.
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Tests ensuring calibration persistence failures preserve the active mapping.
#[cfg(test)]
mod commit_tests {
    use super::*;
    /// Verifies that calibration persistence failure leaves the previously active coordinate
    /// transform unchanged.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn failed_save_keeps_the_existing_map() {
        // Keep four raw corner samples retained for validation and the 17-byte persistence record
        // in this local variable for the following operations.
        let points = [
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 400, y: 500 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 500 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 3500, y: 3600 },
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            RawPoint { x: 400, y: 3600 },
        ];
        // Keep currently used calibration mapping, replaced only after a successful save in this
        // local variable for the following operations.
        let mut active = Calibration::fit(points).unwrap();
        // Keep serialized active mapping retained to verify that a failed save changes nothing in
        // this local variable for the following operations.
        let original = active.encode();
        // Keep proposed value that has not yet replaced the active or persisted baseline in this
        // local variable for the following operations.
        let candidate = Calibration::fit(points.map(|p| RawPoint {
            // Initialize raw horizontal samples or affine coefficients used to map that axis from
            // the supplied value.
            x: 4095 - p.y,
            // Initialize raw vertical samples or affine coefficients used to map that axis from the
            // supplied value.
            y: p.x,
        }))
        .unwrap();
        // Verify the operation failed. A violation means the tested behavior is incorrect.
        assert!(commit_calibration(&mut active, candidate, |_| Err("storage full")).is_err());
        // Verify that serializes the calibration corner samples for the fixed portrait board
        // exactly matches serialized active mapping retained to verify that a failed save changes
        // nothing.
        assert_eq!(active.encode(), original);
        // Keep 17-byte destination receiving the candidate encoding in the successful-save test in
        // this local variable for the following operations.
        let mut saved = [0; 17];
        // Require the value guaranteed by this test fixture or internal invariant; unexpected
        // absence panics.
        commit_calibration(&mut active, candidate, |bytes| {
            // Set 17-byte destination receiving the candidate encoding in the successful-save test
            // to version byte followed by four little-endian x/y pairs.
            saved = *bytes;
            // Run the < , ()> operation with the supplied inputs.
            Ok::<_, ()>(())
        })
        .unwrap();
        // Verify that serializes the calibration corner samples for the fixed portrait board
        // exactly matches 17-byte destination receiving the candidate encoding in the
        // successful-save test.
        assert_eq!(active.encode(), saved);
        // Verify that 17-byte destination receiving the candidate encoding in the successful-save
        // test differs from serialized active mapping retained to verify that a failed save changes
        // nothing.
        assert_ne!(saved, original);
    }
}
