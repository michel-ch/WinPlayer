use biquad::{Biquad, Coefficients, DirectForm1, Q_BUTTERWORTH_F32, ToHertz, Type};

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
        let mut eq = Self {
            enabled: false,
            sample_rate: sample_rate as f32,
            bands_db: [0.0; 10],
            chains: (0..channels.max(1)).map(|_| Self::make_chain(sample_rate as f32, &[0.0; 10])).collect(),
        };
        eq.rebuild();
        eq
    }

    fn make_chain(sample_rate: f32, gains_db: &[f32; 10]) -> [DirectForm1<f32>; 10] {
        std::array::from_fn(|i| {
            let coeffs = Coefficients::<f32>::from_params(
                Type::PeakingEQ(gains_db[i]),
                sample_rate.hz(),
                BAND_FREQS_HZ[i].hz(),
                Q_BUTTERWORTH_F32,
            ).expect("valid coeffs");
            DirectForm1::<f32>::new(coeffs)
        })
    }

    fn rebuild(&mut self) {
        for chain in &mut self.chains {
            for (i, filter) in chain.iter_mut().enumerate() {
                let coeffs = Coefficients::<f32>::from_params(
                    Type::PeakingEQ(self.bands_db[i]),
                    self.sample_rate.hz(),
                    BAND_FREQS_HZ[i].hz(),
                    Q_BUTTERWORTH_F32,
                ).expect("valid coeffs");
                filter.update_coefficients(coeffs);
            }
        }
    }

    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn enabled(&self) -> bool { self.enabled }
    pub fn bands(&self) -> &[f32; 10] { &self.bands_db }
    pub fn set_band(&mut self, idx: usize, db: f32) {
        if idx < 10 { self.bands_db[idx] = db; self.rebuild(); }
    }
    pub fn set_all(&mut self, bands: [f32; 10]) {
        self.bands_db = bands;
        self.rebuild();
    }

    pub fn process_inplace(&mut self, samples: &mut [f32], channels: u16) {
        if !self.enabled { return; }
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
}
