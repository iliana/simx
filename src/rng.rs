use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::array::IntoIter;
use std::num::Wrapping;

type State = [Wrapping<u64>; 2];
type Iter = IntoIter<u64, 64>;

/// XorShift128+, as seen in Node.js 12.
#[derive(Debug, Deserialize, Serialize)]
pub struct Rng {
    state: State,
    #[serde(
        deserialize_with = "deserialize_iter",
        serialize_with = "serialize_iter"
    )]
    iter: Iter,
}

impl Rng {
    pub fn new() -> Rng {
        let mut state = rand_state();
        let iter = next_buf(&mut state);
        Rng { state, iter }
    }

    pub fn seeded(s0: u64, s1: u64) -> Rng {
        let mut state = [Wrapping(s0), Wrapping(s1)];
        let iter = next_buf(&mut state);
        Rng { state, iter }
    }

    pub fn next_f64(&mut self) -> f64 {
        let s0_shifted = if let Some(n) = self.iter.next_back() {
            n
        } else {
            self.iter = next_buf(&mut self.state);
            self.iter
                .next_back()
                .expect("next_buf always produces a 64-element iterator")
        };
        f64::from_bits(s0_shifted | 0x3ff0_0000_0000_0000) - 1.0
    }

    pub fn choose<I>(&mut self, choices: I) -> Option<I::Item>
    where
        I: IntoIterator,
        I::IntoIter: ExactSizeIterator,
    {
        let mut choices = choices.into_iter();
        #[allow(clippy::cast_precision_loss)]
        let len = choices.len() as f64;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (self.next_f64() * len).floor() as usize;
        choices.nth(n)
    }
}

fn rand_state() -> State {
    let mut buf = [0; 16];
    getrandom::getrandom(&mut buf).expect("failed to get random seed");
    // SAFETY: integers are plain old datatypes so we can always transmute to them
    unsafe { std::mem::transmute(buf) }
}

fn next_buf(state: &mut State) -> Iter {
    fn next(state: &mut State) -> u64 {
        let [mut s1, s0] = *state;
        state[0] = s0;
        s1 ^= s1 << 23;
        s1 ^= s1 >> 17;
        s1 ^= s0;
        s1 ^= s0 >> 26;
        *state = [state[1], s1];
        (s0 >> 12).0
    }

    let iter = std::array::from_fn(|_| next(state)).into_iter();
    debug_assert!(iter.size_hint().0 == 64);
    debug_assert!(iter.size_hint().1 == Some(64));
    iter
}

impl Default for Rng {
    fn default() -> Rng {
        Rng::new()
    }
}

impl Iterator for Rng {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        Some(self.next_f64())
    }
}

fn deserialize_iter<'de, D>(deserializer: D) -> Result<Iter, D::Error>
where
    D: Deserializer<'de>,
{
    let mut buf = [0; 64];
    let v: Vec<u64> = Vec::deserialize(deserializer)?;
    let len = buf.len().min(v.len());
    buf[..len].copy_from_slice(&v[..len]);
    let mut iter = buf.into_iter();
    if let Some(n) = (64 - len).checked_sub(1) {
        iter.nth_back(n);
    }
    Ok(iter)
}

fn serialize_iter<S>(iter: &Iter, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    iter.as_slice().serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::Rng;

    impl PartialEq for Rng {
        fn eq(&self, other: &Self) -> bool {
            self.state.eq(&other.state) && self.iter.as_slice().eq(other.iter.as_slice())
        }
    }

    #[allow(clippy::pedantic)]
    #[test]
    fn sixpack() {
        // https://rng.sibr.dev/?state=(9168710189202541577,14545355385888695162)+17
        assert_eq!(
            Rng::seeded(2935246629125674131, 766864515362452477)
                .skip(46)
                .take(26)
                .collect::<Vec<_>>(),
            [
                0.49703677530116863,
                0.5353334247728434,
                0.7801376811715985,
                0.9477862995677102,
                0.5432353904550866,
                0.09148519432489,
                0.13637348943299,
                0.2402088683966459,
                0.7684839792424973,
                0.17754516970112522,
                0.9256864810331178,
                0.47320243374628146,
                0.5251933427814042,
                0.5415813280082218,
                0.05882883251148385,
                0.07467658384889164,
                0.5112415100190766,
                0.04157180867790067,
                0.6657740824633718,
                0.04772255121420832,
                0.22310586243568764,
                0.436032456675467,
                0.46330930297334705,
                0.483643577821103,
                0.8551471045385424,
                0.2681344624704567,
            ]
        );
    }

    #[test]
    fn ser_and_de() {
        let mut rng = Rng::new();
        let rebuilt: Rng = serde_json::from_str(&serde_json::to_string(&rng).unwrap()).unwrap();
        assert_eq!(rng, rebuilt);

        rng.nth(14);
        assert_eq!(rng.iter.as_slice().len(), 49);
        let rebuilt: Rng = serde_json::from_str(&serde_json::to_string(&rng).unwrap()).unwrap();
        assert_eq!(rng, rebuilt);

        rng.nth(48);
        assert_eq!(rng.iter.as_slice().len(), 0);
        let rebuilt: Rng = serde_json::from_str(&serde_json::to_string(&rng).unwrap()).unwrap();
        assert_eq!(rng, rebuilt);

        rng.next();
        assert_eq!(rng.iter.as_slice().len(), 63);
        let rebuilt: Rng = serde_json::from_str(&serde_json::to_string(&rng).unwrap()).unwrap();
        assert_eq!(rng, rebuilt);
    }
}
