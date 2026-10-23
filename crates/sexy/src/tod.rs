//! TodLib animation helpers: curves that drive almost every timed effect in PvZ.
//!
//! Behaviour per ruslan831 decomp, `Sexy.TodLib/TodCommon.cpp`
//! (`TodAnimateCurve`, `TodCurveEvaluate`, `TodCurveEvaluateClamped`).

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodCurve {
    Constant,
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInOutWeak,
    FastInOut,
    FastInOutWeak,
    /// Goes start → end → start.
    Bounce,
    BounceFastMiddle,
    BounceSlowMiddle,
    SinWave,
    EaseSinWave,
}

impl TodCurve {
    /// Curves that end where they started.
    fn returns_to_start(self) -> bool {
        matches!(
            self,
            Self::Bounce | Self::BounceFastMiddle | Self::BounceSlowMiddle | Self::SinWave | Self::EaseSinWave
        )
    }
}

fn quad(t: f32) -> f32 {
    t * t
}

fn inv_quad(t: f32) -> f32 {
    2.0 * t - t * t
}

fn s_curve(t: f32) -> f32 {
    3.0 * t * t - 2.0 * t * t * t
}

fn inv_quad_s(t: f32) -> f32 {
    if t <= 0.5 { inv_quad(t * 2.0) * 0.5 } else { quad((t - 0.5) * 2.0) * 0.5 + 0.5 }
}

fn bounce(t: f32) -> f32 {
    1.0 - (2.0 * t - 1.0).abs()
}

/// Interpolates between `start` and `end` at `t` (unclamped, 0..=1 nominal).
pub fn curve_evaluate(t: f32, start: f32, end: f32, curve: TodCurve) -> f32 {
    let warped = match curve {
        TodCurve::Constant => 0.0,
        TodCurve::Linear => t,
        TodCurve::EaseIn => quad(t),
        TodCurve::EaseOut => inv_quad(t),
        TodCurve::EaseInOut => s_curve(s_curve(t)),
        TodCurve::EaseInOutWeak => s_curve(t),
        TodCurve::FastInOut => inv_quad_s(inv_quad_s(t)),
        TodCurve::FastInOutWeak => inv_quad_s(t),
        TodCurve::Bounce => bounce(t),
        TodCurve::BounceFastMiddle => quad(bounce(t)),
        TodCurve::BounceSlowMiddle => inv_quad(bounce(t)),
        TodCurve::SinWave => (2.0 * PI * t).sin(),
        TodCurve::EaseSinWave => (2.0 * PI * s_curve(t)).sin(),
    };
    (end - start) * warped + start
}

/// Like [`curve_evaluate`], holding the start/end value outside 0..1.
pub fn curve_evaluate_clamped(t: f32, start: f32, end: f32, curve: TodCurve) -> f32 {
    if t <= 0.0 {
        start
    } else if t >= 1.0 {
        if curve.returns_to_start() { start } else { end }
    } else {
        curve_evaluate(t, start, end, curve)
    }
}

/// Rounds half away from zero, like the original's `FloatRoundToInt`.
pub fn float_round_to_int(v: f32) -> i32 {
    if v > 0.0 { (v + 0.5) as i32 } else { (v - 0.5) as i32 }
}

/// Animates over integer ticks. `time_start` may be greater than `time_end`,
/// which is how count-down timers are usually animated.
pub fn animate_curve_float(time_start: i32, time_end: i32, age: i32, start: f32, end: f32, curve: TodCurve) -> f32 {
    let t = (age - time_start) as f32 / (time_end - time_start) as f32;
    curve_evaluate_clamped(t, start, end, curve)
}

pub fn animate_curve(time_start: i32, time_end: i32, age: i32, start: i32, end: i32, curve: TodCurve) -> i32 {
    float_round_to_int(animate_curve_float(time_start, time_end, age, start as f32, end as f32, curve))
}

/// [`animate_curve_float`] over a continuous time value.
pub fn animate_curve_float_time(time_start: f32, time_end: f32, age: f32, start: f32, end: f32, curve: TodCurve) -> f32 {
    curve_evaluate_clamped((age - time_start) / (time_end - time_start), start, end, curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_counts_down() {
        // Typical use: a counter running from 50 down to 0 fades 255 -> 0.
        assert_eq!(animate_curve(50, 0, 50, 255, 0, TodCurve::Linear), 255);
        assert_eq!(animate_curve(50, 0, 25, 255, 0, TodCurve::Linear), 128);
        assert_eq!(animate_curve(50, 0, 0, 255, 0, TodCurve::Linear), 0);
        assert_eq!(animate_curve(50, 0, -10, 255, 0, TodCurve::Linear), 0);
    }

    #[test]
    fn bounce_returns_to_start() {
        assert_eq!(animate_curve(60, 50, 55, 10, 15, TodCurve::Bounce), 15);
        assert_eq!(animate_curve(60, 50, 50, 10, 15, TodCurve::Bounce), 10);
        assert_eq!(animate_curve(60, 50, 40, 10, 15, TodCurve::Bounce), 10);
    }

    #[test]
    fn ease_in_is_quadratic() {
        assert_eq!(curve_evaluate(0.5, 0.0, 100.0, TodCurve::EaseIn), 25.0);
        assert_eq!(curve_evaluate(0.5, 0.0, 100.0, TodCurve::EaseOut), 75.0);
    }

    #[test]
    fn rounds_half_away_from_zero() {
        assert_eq!(float_round_to_int(2.5), 3);
        assert_eq!(float_round_to_int(-2.5), -3);
        assert_eq!(float_round_to_int(-0.4), 0);
    }
}
