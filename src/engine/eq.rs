use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};

pub const BAND_FREQS_HZ: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub struct Equalizer {
    enabled: bool,
    sample_rate: f32,
    bands_db: [f32; 10],
    chains: Vec<[DirectForm1<f32>; 10]>,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let sample_rate = sanitize_sample_rate(sample_rate as f32);
        let mut eq = Self {
            enabled: false,
            sample_rate,
            bands_db: [0.0; 10],
            chains: (0..channels.max(1))
                .map(|_| Self::make_chain(sample_rate, &[0.0; 10]))
                .collect(),
        };
        eq.rebuild();
        eq
    }

    fn make_chain(sample_rate: f32, gains_db: &[f32; 10]) -> [DirectForm1<f32>; 10] {
        std::array::from_fn(|i| {
            let coeffs = safe_coefficients(sample_rate, BAND_FREQS_HZ[i], gains_db[i]);
            DirectForm1::<f32>::new(coeffs)
        })
    }

    fn rebuild(&mut self) {
        for chain in &mut self.chains {
            for (i, filter) in chain.iter_mut().enumerate() {
                let coeffs =
                    safe_coefficients(self.sample_rate, BAND_FREQS_HZ[i], self.bands_db[i]);
                filter.update_coefficients(coeffs);
            }
        }
    }

    pub fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn bands(&self) -> &[f32; 10] {
        &self.bands_db
    }
    pub fn set_band(&mut self, idx: usize, db: f32) {
        if idx < 10 {
            self.bands_db[idx] = sanitize_gain(db);
            self.rebuild();
        }
    }
    pub fn set_all(&mut self, bands: [f32; 10]) {
        self.bands_db = bands.map(sanitize_gain);
        self.rebuild();
    }

    pub fn process_inplace(&mut self, samples: &mut [f32], channels: u16) {
        if !self.enabled {
            return;
        }
        let n_chans = channels.max(1) as usize;
        for (i, s) in samples.iter_mut().enumerate() {
            let ch = i % n_chans;
            let chain_idx = ch.min(self.chains.len() - 1);
            for filter in &mut self.chains[chain_idx] {
                *s = filter.run(*s);
            }
        }
    }
}

fn sanitize_sample_rate(sample_rate: f32) -> f32 {
    if sample_rate.is_finite() && sample_rate > 1.0 {
        sample_rate
    } else {
        44_100.0
    }
}

fn sanitize_gain(db: f32) -> f32 {
    if db.is_finite() {
        db.clamp(-24.0, 24.0)
    } else {
        0.0
    }
}

fn safe_coefficients(sample_rate: f32, freq: f32, db: f32) -> Coefficients<f32> {
    let sample_rate = sanitize_sample_rate(sample_rate);
    let freq = freq.clamp(1.0, sample_rate * 0.49);
    let db = sanitize_gain(db);
    match Coefficients::<f32>::from_params(
        Type::PeakingEQ(db),
        sample_rate.hz(),
        freq.hz(),
        Q_BUTTERWORTH_F32,
    ) {
        Ok(coeffs) => coeffs,
        Err(_) => Coefficients::<f32>::from_params(
            Type::PeakingEQ(0.0),
            44_100.0.hz(),
            1_000.0.hz(),
            Q_BUTTERWORTH_F32,
        )
        .unwrap_or_else(|_| unreachable!("static fallback EQ coefficients are valid")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_is_noop() {
        let mut eq = Equalizer::new(44_100, 2);
        eq.set_band(0, 12.0);
        let mut samples = vec![0.5_f32, -0.5, 0.5, -0.5];
        let copy = samples.clone();
        eq.process_inplace(&mut samples, 2);
        assert_eq!(samples, copy);
    }

    #[test]
    fn enabled_modifies_signal() {
        let mut eq = Equalizer::new(44_100, 2);
        eq.set_enabled(true);
        eq.set_band(5, 12.0);
        let mut samples = vec![0.5_f32; 1024];
        eq.process_inplace(&mut samples, 2);
        assert!(samples.iter().any(|s| (*s - 0.5).abs() > 0.001));
    }

    #[test]
    fn invalid_rates_and_gains_do_not_panic_or_emit_nan() {
        let mut eq = Equalizer::new(0, 2);
        eq.set_enabled(true);
        eq.set_all([
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            96.0,
            -96.0,
            0.0,
            12.0,
            -12.0,
            6.0,
            -6.0,
        ]);

        let mut samples = vec![0.25_f32; 64];
        eq.process_inplace(&mut samples, 2);

        assert!(samples.iter().all(|s| s.is_finite()));
    }
}
