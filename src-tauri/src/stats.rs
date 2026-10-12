//! Kennzahlen aus Messreihen: Median, Perzentil, Jitter, Verlust.

/// Median (bei gerader Anzahl Mittel der beiden mittleren Werte). Leere Reihe -> 0.
pub fn median(v: &[f64]) -> f64 {
    percentile(v, 50.0)
}

/// Perzentil mit linearer Interpolation (p in 0..=100). Leere Reihe -> 0.
pub fn percentile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return 0.0;
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rank = (p / 100.0).clamp(0.0, 1.0) * (s.len() - 1) as f64;
    let (lo, hi) = (rank.floor() as usize, rank.ceil() as usize);
    s[lo] + (s[hi] - s[lo]) * (rank - lo as f64)
}

pub fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(0.0, f64::max)
}

pub fn min(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().copied().fold(f64::INFINITY, f64::min) }
}

/// Jitter = mittlere Abweichung aufeinanderfolgender Laufzeiten (wie bei gängigen Speedtests).
pub fn jitter(rtts: &[f64]) -> f64 {
    if rtts.len() < 2 {
        return 0.0;
    }
    rtts.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (rtts.len() - 1) as f64
}

/// Durchsatz in Mbit/s aus Bytes und Sekunden.
pub fn mbps(bytes: u64, secs: f64) -> f64 {
    if secs <= 0.0 { 0.0 } else { bytes as f64 * 8.0 / secs / 1_000_000.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kennzahlen() {
        assert_eq!(median(&[]), 0.0);
        assert_eq!(median(&[5.0]), 5.0);
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], 100.0), 5.0);
        assert!((percentile(&[1.0, 2.0, 3.0, 4.0, 5.0], 95.0) - 4.8).abs() < 1e-9);
        assert_eq!(jitter(&[10.0]), 0.0);
        assert_eq!(jitter(&[10.0, 12.0, 11.0, 15.0]), (2.0 + 1.0 + 4.0) / 3.0);
        assert_eq!(mbps(125_000_000, 1.0), 1000.0);
        assert_eq!(mbps(1, 0.0), 0.0);
        assert_eq!(max(&[1.0, 9.0, 3.0]), 9.0);
        assert_eq!(min(&[4.0, 2.0, 3.0]), 2.0);
    }
}
