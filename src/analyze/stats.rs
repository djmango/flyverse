//! Scalar statistics helpers.
//!
//! The f32 family (`mean`, `pct`, `pearson`) runs on the trace and feeds the
//! JSON summary; the f64 family (`mean64`, `sem64`, `welch_t`) runs on the
//! probe's per-trial scalars. They are kept separate on purpose: unifying them
//! would change the arithmetic and can move printed numbers.

// ---------------------------------------------------------------------------
// Stats helpers

pub(crate) fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().map(|&x| x as f64).sum::<f64>() as f32 / v.len() as f32
}

pub(crate) fn pct(v: &[f32], q: f64) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((s.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    s[i]
}

pub(crate) fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let ma = a[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let mb = b[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let (mut num, mut da, mut db) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let x = a[i] as f64 - ma;
        let y = b[i] as f64 - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    let d = (da * db).sqrt();
    if d <= 1e-12 {
        0.0
    } else {
        (num / d) as f32
    }
}

/// xorshift64. Deterministic from the seed, so a probe run is reproducible.
pub(crate) fn next_u64(r: &mut u64) -> u64 {
    *r ^= *r << 13;
    *r ^= *r >> 7;
    *r ^= *r << 17;
    *r
}

pub(crate) fn mean64(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

pub(crate) fn sem64(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::INFINITY;
    }
    let m = mean64(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64;
    (var / v.len() as f64).sqrt()
}

/// Welch's t statistic for a difference of means. Reported with the caveat that
/// the trials are not independent samples of a stationary process, so the number
/// is a signal-to-noise guide rather than a p-value.
pub(crate) fn welch_t(a: &[f64], b: &[f64]) -> f64 {
    let (sa, sb) = (sem64(a), sem64(b));
    let denom = (sa * sa + sb * sb).sqrt();
    if !denom.is_finite() || denom <= 0.0 {
        return 0.0;
    }
    ((mean64(a) - mean64(b)) / denom).abs()
}

/// Balanced sign schedule: one list per probed axis, each half positive and half
/// negative, shuffled with the seeded generator so the sign is uncorrelated with
/// the fly's own behaviour at that instant. Equal group sizes matter because the
/// two means are compared directly, and with few trials an unbalanced draw would
/// put most of the weight on one side.
pub(crate) fn balanced_sign_schedule(n_axes: usize, per_axis: usize, seed: u64) -> Vec<Vec<bool>> {
    let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
    (0..n_axes)
        .map(|_| {
            let mut v: Vec<bool> = (0..per_axis).map(|i| i % 2 == 0).collect();
            for i in (1..v.len()).rev() {
                let j = (next_u64(&mut rng) % (i as u64 + 1)) as usize;
                v.swap(i, j);
            }
            v
        })
        .collect()
}