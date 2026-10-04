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
pub const TARGETS: [(i32, i32); 4] = [(24, 24), (295, 24), (295, 455), (24, 455)];
pub const CENTER: (i32, i32) = (160, 240);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawPoint {
    pub x: u16,
    pub y: u16,
}
impl RawPoint {
    fn valid(self) -> bool {
        self.x > 0 && self.x < 4095 && self.y > 0 && self.y < 4095
    }
}

/// Five conversions suppress ADC noise. Reject rail values, weak contact, and
/// samples that move too far during one measurement.
pub fn filter(samples: [(RawPoint, u16); 5]) -> Option<RawPoint> {
    let mut x = [0; 5];
    let mut y = [0; 5];
    let mut pressure = [0; 5];
    for (i, (point, z)) in samples.into_iter().enumerate() {
        if !point.valid() {
            return None;
        }
        x[i] = point.x;
        y[i] = point.y;
        pressure[i] = z;
    }
    x.sort_unstable();
    y.sort_unstable();
    pressure.sort_unstable();
    if pressure[2] < 600 || x[4] - x[0] > 150 || y[4] - y[0] > 150 {
        return None;
    }
    Some(RawPoint { x: x[2], y: y[2] })
}

/// Require two contact readings and three release readings, avoiding repeated
/// actions when a held finger briefly produces a weak sample.
#[derive(Default)]
pub struct Press {
    held: bool,
    contact: u8,
    release: u8,
}
impl Press {
    pub fn update(&mut self, point: Option<RawPoint>) -> Option<RawPoint> {
        if let Some(point) = point {
            self.release = 0;
            self.contact = self.contact.saturating_add(1);
            if !self.held && self.contact >= 2 {
                self.held = true;
                return Some(point);
            }
        } else {
            self.contact = 0;
            self.release = self.release.saturating_add(1);
            if self.release >= 3 {
                self.held = false;
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
pub struct Calibration {
    points: [RawPoint; 4],
    x: [i64; 3],
    y: [i64; 3],
    divisor: i64,
}
impl Calibration {
    pub fn fit(points: [RawPoint; 4]) -> Option<Self> {
        if !points.iter().all(|p| p.valid()) {
            return None;
        }
        let [a, b, c, _] = points;
        let (ax, ay, bx, by, cx, cy) = (
            a.x as i64, a.y as i64, b.x as i64, b.y as i64, c.x as i64, c.y as i64,
        );
        let divisor = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
        if divisor.abs() < 200_000 {
            return None;
        }
        let coefficients = |values: [i64; 3]| {
            let u = (values[1] - values[0]) * (cy - ay) - (values[2] - values[0]) * (by - ay);
            let v = (bx - ax) * (values[2] - values[0]) - (cx - ax) * (values[1] - values[0]);
            [u, v, values[0] * divisor - u * ax - v * ay]
        };
        let cal = Self {
            points,
            x: coefficients([24, 295, 295]),
            y: coefficients([24, 24, 455]),
            divisor,
        };
        let fourth = cal.project(points[3]);
        if (fourth.0 - TARGETS[3].0).abs() > 20 || (fourth.1 - TARGETS[3].1).abs() > 20 {
            return None;
        }
        Some(cal)
    }
    fn project(&self, p: RawPoint) -> (i32, i32) {
        let axis =
            |c: [i64; 3]| ((c[0] * p.x as i64 + c[1] * p.y as i64 + c[2]) / self.divisor) as i32;
        (axis(self.x), axis(self.y))
    }
    pub fn map(&self, p: RawPoint) -> Option<(i32, i32)> {
        if !p.valid() {
            return None;
        }
        let (x, y) = self.project(p);
        if !(-10..330).contains(&x) || !(-10..490).contains(&y) {
            return None;
        }
        Some((x.clamp(0, 319), y.clamp(0, 479)))
    }
    pub fn verify_center(&self, p: RawPoint) -> bool {
        self.map(p)
            .is_some_and(|(x, y)| (x - CENTER.0).abs() <= 20 && (y - CENTER.1).abs() <= 20)
    }
    /// Version 1 is bound to this board's fixed portrait orientation.
    pub fn encode(&self) -> [u8; 17] {
        let mut bytes = [0; 17];
        bytes[0] = 1;
        for (point, out) in self.points.iter().zip(bytes[1..].chunks_exact_mut(4)) {
            out[..2].copy_from_slice(&point.x.to_le_bytes());
            out[2..].copy_from_slice(&point.y.to_le_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 17 || bytes[0] != 1 {
            return None;
        }
        let mut points = [RawPoint { x: 0, y: 0 }; 4];
        for (point, chunk) in points.iter_mut().zip(bytes[1..].chunks_exact(4)) {
            point.x = u16::from_le_bytes([chunk[0], chunk[1]]);
            point.y = u16::from_le_bytes([chunk[2], chunk[3]]);
        }
        Self::fit(points)
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    fn corners() -> [RawPoint; 4] {
        [
            RawPoint { x: 400, y: 500 },
            RawPoint { x: 3500, y: 500 },
            RawPoint { x: 3500, y: 3600 },
            RawPoint { x: 400, y: 3600 },
        ]
    }
    #[test]
    fn maps_corners_center_and_restarts() {
        let cal = Calibration::fit(corners()).unwrap();
        for (p, target) in corners().into_iter().zip(TARGETS) {
            assert_eq!(cal.map(p), Some(target));
        }
        assert!(cal.verify_center(RawPoint { x: 1955, y: 2050 }));
        assert!(!cal.verify_center(corners()[0]));
        let restored = Calibration::decode(&cal.encode()).unwrap();
        assert_eq!(restored.map(corners()[2]), Some(TARGETS[2]));
    }
    #[test]
    fn swapped_and_inverted_axes_map_correctly() {
        let points = corners().map(|p| RawPoint {
            x: 4095 - p.y,
            y: p.x,
        });
        let cal = Calibration::fit(points).unwrap();
        for (p, target) in points.into_iter().zip(TARGETS) {
            assert_eq!(cal.map(p), Some(target));
        }
    }
    #[test]
    fn invalid_calibration_and_outside_touches_are_rejected() {
        assert!(Calibration::fit([corners()[0]; 4]).is_none());
        let mut points = corners();
        points[3] = points[2];
        assert!(Calibration::fit(points).is_none());
        assert!(Calibration::decode(&[0; 17]).is_none());
        let cal = Calibration::fit(corners()).unwrap();
        assert!(Calibration::decode(&cal.encode()[..16]).is_none());
        assert!(cal.map(RawPoint { x: 1, y: 1 }).is_none());
        assert!(cal.map(RawPoint { x: 0, y: 500 }).is_none());
    }
    #[test]
    fn noise_pressure_and_disconnected_adc_are_rejected() {
        let p = RawPoint { x: 1000, y: 2000 };
        assert_eq!(filter([(p, 800); 5]), Some(p));
        assert!(filter([(p, 100); 5]).is_none());
        assert!(filter([(RawPoint { x: 0, y: 0 }, 4095); 5]).is_none());
        let mut samples = [(p, 800); 5];
        samples[0].0.x = 1200;
        assert!(filter(samples).is_none());
    }
    #[test]
    fn one_action_per_press_survives_brief_contact_loss() {
        let p = RawPoint { x: 1000, y: 2000 };
        let mut press = Press::default();
        assert!(press.update(Some(p)).is_none());
        assert_eq!(press.update(Some(p)), Some(p));
        assert!(press.update(None).is_none());
        assert!(press.update(Some(p)).is_none());
        for _ in 0..3 {
            assert!(press.update(None).is_none());
        }
        assert!(press.update(Some(p)).is_none());
        assert_eq!(press.update(Some(p)), Some(p));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down(RawPoint),
    Move(RawPoint),
    Up,
}
impl Press {
    pub fn until_release() -> Self {
        Self {
            held: true,
            contact: 0,
            release: 0,
        }
    }
    pub fn event(&mut self, point: Option<RawPoint>) -> Option<TouchPhase> {
        let held = self.held;
        if let Some(point) = self.update(point) {
            return Some(TouchPhase::Down(point));
        }
        if held && !self.held {
            return Some(TouchPhase::Up);
        }
        if self.held {
            return point.map(TouchPhase::Move);
        }
        None
    }
}

pub struct Wizard {
    pub stage: usize,
    points: [RawPoint; 4],
    candidate: Option<Calibration>,
    deadline: u64,
    press: Press,
}
pub enum WizardResult {
    Waiting,
    Advanced,
    Verified(Calibration),
    Invalid,
    TimedOut,
    Cancelled,
}
impl Wizard {
    pub fn new(now: u64) -> Self {
        Self {
            stage: 0,
            points: [RawPoint { x: 0, y: 0 }; 4],
            candidate: None,
            deadline: now + 60_000,
            press: Press::until_release(),
        }
    }
    pub fn target(&self) -> (i32, i32) {
        if self.stage < 4 {
            TARGETS[self.stage]
        } else {
            CENTER
        }
    }
    pub fn update(&mut self, point: Option<RawPoint>, cancel: bool, now: u64) -> WizardResult {
        if cancel {
            return WizardResult::Cancelled;
        }
        if now >= self.deadline {
            return WizardResult::TimedOut;
        }
        let Some(point) = self.press.update(point) else {
            return WizardResult::Waiting;
        };
        if self.stage < 4 {
            self.points[self.stage] = point;
            self.stage += 1;
            self.deadline = now + 60_000;
            if self.stage == 4 {
                self.candidate = Calibration::fit(self.points);
                if self.candidate.is_none() {
                    return WizardResult::Invalid;
                }
            }
            WizardResult::Advanced
        } else {
            match self.candidate.filter(|cal| cal.verify_center(point)) {
                Some(cal) => WizardResult::Verified(cal),
                None => WizardResult::Invalid,
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod wizard_tests {
    use super::*;
    fn release(wizard: &mut Wizard, now: u64) {
        for _ in 0..3 {
            wizard.update(None, false, now);
        }
    }
    fn press(wizard: &mut Wizard, point: RawPoint, now: u64) -> WizardResult {
        wizard.update(Some(point), false, now);
        wizard.update(Some(point), false, now)
    }
    #[test]
    fn wizard_waits_for_release_and_verifies_before_saving() {
        let mut wizard = Wizard::new(0);
        let point = RawPoint { x: 400, y: 500 };
        assert!(matches!(
            press(&mut wizard, point, 10),
            WizardResult::Waiting
        ));
        for point in [
            point,
            RawPoint { x: 3500, y: 500 },
            RawPoint { x: 3500, y: 3600 },
            RawPoint { x: 400, y: 3600 },
        ] {
            release(&mut wizard, 20);
            assert!(matches!(
                press(&mut wizard, point, 30),
                WizardResult::Advanced
            ));
            assert!(matches!(
                press(&mut wizard, point, 40),
                WizardResult::Waiting
            ));
        }
        assert_eq!(wizard.target(), CENTER);
        release(&mut wizard, 50);
        assert!(matches!(
            press(&mut wizard, RawPoint { x: 1955, y: 2050 }, 60),
            WizardResult::Verified(_)
        ));
    }
    #[test]
    fn cancel_timeout_and_bad_geometry_never_produce_a_candidate() {
        assert!(matches!(
            Wizard::new(0).update(None, true, 10),
            WizardResult::Cancelled
        ));
        assert!(matches!(
            Wizard::new(0).update(None, false, 60_000),
            WizardResult::TimedOut
        ));
        let mut wizard = Wizard::new(0);
        for i in 0..4 {
            release(&mut wizard, 10);
            let result = press(&mut wizard, RawPoint { x: 400, y: 500 }, 20);
            if i == 3 {
                assert!(matches!(result, WizardResult::Invalid));
            }
        }
    }
    #[test]
    fn touch_phases_keep_button_debounce_and_report_drag_and_release() {
        let mut press = Press::default();
        let p = RawPoint { x: 1000, y: 1000 };
        assert_eq!(press.event(Some(p)), None);
        assert_eq!(press.event(Some(p)), Some(TouchPhase::Down(p)));
        assert_eq!(press.event(Some(p)), Some(TouchPhase::Move(p)));
        assert_eq!(press.event(None), None);
        assert_eq!(press.event(None), None);
        assert_eq!(press.event(None), Some(TouchPhase::Up));
    }
}

/// The active map changes only after the persistence callback succeeds.
pub fn commit_calibration<E>(
    active: &mut Calibration,
    candidate: Calibration,
    write: impl FnOnce(&[u8; 17]) -> Result<(), E>,
) -> Result<(), E> {
    write(&candidate.encode())?;
    *active = candidate;
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod commit_tests {
    use super::*;
    #[test]
    fn failed_save_keeps_the_existing_map() {
        let points = [
            RawPoint { x: 400, y: 500 },
            RawPoint { x: 3500, y: 500 },
            RawPoint { x: 3500, y: 3600 },
            RawPoint { x: 400, y: 3600 },
        ];
        let mut active = Calibration::fit(points).unwrap();
        let original = active.encode();
        let candidate = Calibration::fit(points.map(|p| RawPoint {
            x: 4095 - p.y,
            y: p.x,
        }))
        .unwrap();
        assert!(commit_calibration(&mut active, candidate, |_| Err("storage full")).is_err());
        assert_eq!(active.encode(), original);
        let mut saved = [0; 17];
        commit_calibration(&mut active, candidate, |bytes| {
            saved = *bytes;
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(active.encode(), saved);
        assert_ne!(saved, original);
    }
}
