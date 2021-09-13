const NTAB: i32 = 32;
const IA: i32 = 16807;
const IM: i32 = i32::MAX;
const IQ: i32 = 127_773;
const IR: i32 = 2836;
const NDIV: i32 = 1 + (IM - 1) / NTAB;
const MAX_RANDOM_RANGE: i32 = IM;

const AM: f64 = 1.0 / IM as f64;
const EPS: f64 = 1.2e-7;
const RNMX: f64 = 1.0 - EPS;

/// The Uniform Random Number Generator
///
/// Should be instantiated with the `with_seed` method.
///
/// # Usage
///
/// ```
/// use valve_sdk13_rng::UniformRandomStream;
///
/// let mut gen = UniformRandomStream::with_seed(72);
/// let res = gen.random_f64(0_f64, 1_f64);
/// assert_eq!(0.543_099_8, res);
/// ```
///
#[derive(Debug, Clone)]
pub struct UniformRandomStream {
    pub m_idum: i32,
    pub m_iy: i32,
    pub m_iv: Vec<i32>,
}

impl UniformRandomStream {
    pub fn with_seed(i_seed: i32) -> Self {
        let midum = -i_seed.abs();

        Self {
            m_idum: midum,
            m_iy: 0,
            m_iv: vec![0; NTAB as usize],
        }
    }

    fn generate_random_number(&mut self) -> i32 {
        let mut k;
        let mut j;

        let mut m_idum = self.m_idum;
        let mut m_iy = self.m_iy;
        let m_iv = &mut self.m_iv;

        if m_idum <= 0 || m_iy == 0 {
            if -m_idum < 1 {
                m_idum = 1;
            } else {
                m_idum = -m_idum;
            }

            j = NTAB + 7;
            while j >= 0 {
                k = m_idum / IQ;
                m_idum = IA * (m_idum - k * IQ) - IR * k;

                if m_idum < 0 {
                    m_idum += IM;
                }

                if j < NTAB {
                    m_iv[j as usize] = m_idum;
                }

                j -= 1;
            }

            m_iy = m_iv[0];
        }

        k = m_idum / IQ;
        m_idum = IA * (m_idum - k * IQ) - IR * k;

        if m_idum < 0 {
            m_idum += IM;
        }

        let j = m_iy / NDIV;

        m_iy = m_iv[j as usize];
        m_iv[j as usize] = m_idum;

        self.m_idum = m_idum;
        self.m_iy = m_iy;

        m_iy
    }

    pub fn random_f64(&mut self, low: f64, high: f64) -> f64 {
        let mut fl = AM * f64::from(self.generate_random_number());

        if fl > RNMX {
            fl = RNMX;
        }

        fl.mul_add(high - low, low)
    }

    pub fn random_f64_exp(&mut self, low: f64, high: f64, exponent: f64) -> f64 {
        let mut fl = AM * f64::from(self.generate_random_number());

        if fl > RNMX {
            fl = RNMX;
        }

        if exponent != 1.0 {
            fl = fl.powf(exponent);
        }

        fl.mul_add(high - low, low)
    }

    pub fn random_i32(&mut self, low: i32, high: i32) -> i32 {
        let x = high - low + 1;

        if x <= 1 {
            return low;
        }

        // From Source Engine 2007:
        // The following maps a uniform distribution on the interval [0,MAX_RANDOM_RANGE]
        // to a smaller, client-specified range of [0,x-1] in a way that doesn't bias
        // the uniform distribution unfavorably. Even for a worst case x, the loop is
        // guaranteed to be taken no more than half the time, so for that worst case x,
        // the average number of times through the loop is 2. For cases where x is
        // much smaller than MAX_RANDOM_RANGE, the average number of times through the
        // loop is very close to 1.
        let x1 = MAX_RANDOM_RANGE.wrapping_add(1) % x;
        let max_acceptable = MAX_RANDOM_RANGE.saturating_sub(x1);

        let number = loop {
            let n = self.generate_random_number();

            if n <= max_acceptable {
                break n;
            }
        };

        low + (number % x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_random_float() {
        let proper: Vec<f32> = vec![0.543_099_8, 0.406_318_28, 62.147_213, 0.058_990_162];

        let mut u_rng = UniformRandomStream::with_seed(72);
        let results = vec![
            u_rng.random_f64(0.0, 1.0) as f32,
            u_rng.random_f64(0.0, 1.0) as f32,
            u_rng.random_f64(0.0, 100.0) as f32,
            u_rng.random_f64(0.0, 1.0) as f32,
        ];
        assert_eq!(proper, results);
    }

    #[test]
    fn t_random_int() {
        let proper: Vec<i32> = vec![6, 9, 95, 8];

        let mut u_rng = UniformRandomStream::with_seed(555);
        let results = vec![
            u_rng.random_i32(0, 10),
            u_rng.random_i32(0, 10),
            u_rng.random_i32(0, 100),
            u_rng.random_i32(0, 10),
        ];
        assert_eq!(proper, results);
    }
}
