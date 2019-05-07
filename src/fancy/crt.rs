//! Module containing `CrtGadgets`, which are the CRT-based gadgets for `Fancy`.

use super::{Fancy, HasModulus};
use crate::error::FancyError;
use crate::fancy::bundle::{Bundle, BundleGadgets};
use crate::util;
use itertools::Itertools;
use std::ops::Deref;

/// Bundle which is explicitly CRT-representation.
#[derive(Clone)]
pub struct CrtBundle<W: Clone + HasModulus>(Bundle<W>);

impl<W: Clone + HasModulus> CrtBundle<W> {
    /// Create a new CRT bundle from a vector of wires.
    pub fn new(ws: Vec<W>) -> CrtBundle<W> {
        CrtBundle(Bundle::new(ws))
    }

    /// Extract the underlying bundle from this CRT bundle.
    pub fn extract(self) -> Bundle<W> {
        self.0
    }
}

impl<W: Clone + HasModulus> Deref for CrtBundle<W> {
    type Target = Bundle<W>;

    fn deref(&self) -> &Bundle<W> {
        &self.0
    }
}

impl<W: Clone + HasModulus> From<Bundle<W>> for CrtBundle<W> {
    fn from(b: Bundle<W>) -> CrtBundle<W> {
        CrtBundle(b)
    }
}

impl<F: Fancy> CrtGadgets for F {}

/// Extension trait for `Fancy` providing advanced CRT gadgets based on bundles of wires.
pub trait CrtGadgets: Fancy + BundleGadgets {
    /// Initialize a fancy object using CRT bundles for convenience. Can only be called once.
    fn crt_init(
        &mut self,
        garbler_mods: &[u128],
        evaluator_mods: &[u128],
        reused_deltas: &[Self::Item],
    ) -> Result<(Vec<CrtBundle<Self::Item>>, Vec<CrtBundle<Self::Item>>), Self::Error> {
        let gb_ms = garbler_mods.iter().map(|q| util::factor(*q)).collect_vec();
        let ev_ms = evaluator_mods
            .iter()
            .map(|q| util::factor(*q))
            .collect_vec();

        let (xs, ys) = self.init_bundles(&gb_ms, &ev_ms, reused_deltas)?;

        Ok((
            xs.into_iter().map(CrtBundle::from).collect_vec(),
            ys.into_iter().map(CrtBundle::from).collect_vec(),
        ))
    }

    /// Creates a bundle of constant wires for the CRT representation of `x` under
    /// composite modulus `q`.
    fn crt_constant_bundle(
        &mut self,
        x: u128,
        q: u128,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        let ps = util::factor(q);
        let xs = ps.iter().map(|&p| (x % p as u128) as u16).collect_vec();
        self.constant_bundle(&xs, &ps).map(CrtBundle)
    }

    /// Output a slice of CRT bundles.
    fn crt_outputs(&mut self, xs: &[CrtBundle<Self::Item>]) -> Result<(), Self::Error> {
        for x in xs.iter() {
            self.output_bundle(x)?;
        }
        Ok(())
    }

    ////////////////////////////////////////////////////////////////////////////////
    // High-level computations dealing with bundles.

    /// Add two CRT bundles.
    fn crt_add(
        &mut self,
        x: &CrtBundle<Self::Item>,
        y: &CrtBundle<Self::Item>,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        self.add_bundles(x, y).map(CrtBundle)
    }

    /// Subtract two CRT bundles.
    fn crt_sub(
        &mut self,
        x: &CrtBundle<Self::Item>,
        y: &CrtBundle<Self::Item>,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        self.sub_bundles(x, y).map(CrtBundle)
    }

    /// Multiplies each wire in `x` by the corresponding residue of `c`.
    fn crt_cmul(
        &mut self,
        x: &CrtBundle<Self::Item>,
        c: u128,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        let cs = util::crt(c, &x.moduli());
        x.wires()
            .iter()
            .zip(cs.into_iter())
            .map(|(x, c)| self.cmul(x, c))
            .collect::<Result<Vec<Self::Item>, Self::Error>>()
            .map(CrtBundle::new)
    }

    /// Multiply `x` with `y`.
    fn crt_mul(
        &mut self,
        x: &CrtBundle<Self::Item>,
        y: &CrtBundle<Self::Item>,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        self.mul_bundles(x, y).map(CrtBundle)
    }

    /// Exponentiate `x` by the constant `c`.
    fn crt_cexp(
        &mut self,
        x: &CrtBundle<Self::Item>,
        c: u16,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        x.wires()
            .iter()
            .map(|x| {
                let p = x.modulus();
                let tab = (0..p)
                    .map(|x| ((x as u64).pow(c as u32) % p as u64) as u16)
                    .collect_vec();
                self.proj(x, p, Some(tab))
            })
            .collect::<Result<Vec<Self::Item>, Self::Error>>()
            .map(CrtBundle::new)
    }

    /// Compute the remainder with respect to modulus `p`.
    fn crt_rem(
        &mut self,
        x: &CrtBundle<Self::Item>,
        p: u16,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        let i = x.moduli().iter().position(|&q| p == q).ok_or_else(|| {
            Self::Error::from(FancyError::InvalidArg(
                "p is not a modulus in this bundle!".to_string(),
            ))
        })?;
        let w = &x.wires()[i];
        x.moduli()
            .iter()
            .map(|&q| self.mod_change(w, q))
            .collect::<Result<Vec<Self::Item>, Self::Error>>()
            .map(CrtBundle::new)
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Fancy functions based on Mike's fractional mixed radix trick.

    /// Helper function for advanced gadgets, returns the MSB of the fractional part of
    /// `X/M` where `M=product(ms)`.
    fn crt_fractional_mixed_radix(
        &mut self,
        bun: &CrtBundle<Self::Item>,
        ms: &[u16],
    ) -> Result<Self::Item, Self::Error> {
        let ndigits = ms.len();

        let q = util::product(&bun.moduli());
        let M = util::product(ms);

        let mut ds = Vec::new();

        for wire in bun.wires().iter() {
            let p = wire.modulus();

            let mut tabs = vec![Vec::with_capacity(p as usize); ndigits];

            for x in 0..p {
                let crt_coef = util::inv(((q / p as u128) % p as u128) as i128, p as i128);
                let y = (M as f64 * x as f64 * crt_coef as f64 / p as f64).round() as u128 % M;
                let digits = util::as_mixed_radix(y, ms);
                for i in 0..ndigits {
                    tabs[i].push(digits[i]);
                }
            }

            let new_ds = tabs
                .into_iter()
                .enumerate()
                .map(|(i, tt)| self.proj(wire, ms[i], Some(tt)))
                .collect::<Result<Vec<Self::Item>, Self::Error>>()?;

            ds.push(Bundle::new(new_ds));
        }

        self.mixed_radix_addition_msb_only(&ds)
    }

    /// Compute `max(x,0)`.
    ///
    /// Optional output moduli.
    fn crt_relu(
        &mut self,
        x: &CrtBundle<Self::Item>,
        accuracy: &str,
        output_moduli: Option<&[u16]>,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        let factors_of_m = &get_ms(x, accuracy);
        let res = self.crt_fractional_mixed_radix(x, factors_of_m)?;

        // project the MSB to 0/1, whether or not it is less than p/2
        let p = *factors_of_m.last().unwrap();
        let mask_tt = (0..p).map(|x| (x < p / 2) as u16).collect_vec();
        let mask = self.proj(&res, 2, Some(mask_tt))?;

        // use the mask to either output x or 0
        output_moduli
            .map(|ps| x.with_moduli(ps))
            .as_ref()
            .unwrap_or(x)
            .wires()
            .iter()
            .map(|x| self.mul(x, &mask))
            .collect::<Result<Vec<Self::Item>, Self::Error>>()
            .map(CrtBundle::new)
    }

    /// Return 0 if `x` is positive and 1 if `x` is negative.
    fn crt_sign(
        &mut self,
        x: &CrtBundle<Self::Item>,
        accuracy: &str,
    ) -> Result<Self::Item, Self::Error> {
        let factors_of_m = &get_ms(x, accuracy);
        let res = self.crt_fractional_mixed_radix(x, factors_of_m)?;
        let p = *factors_of_m.last().unwrap();
        let tt = (0..p).map(|x| (x >= p / 2) as u16).collect_vec();
        self.proj(&res, 2, Some(tt))
    }

    /// Return `if x >= 0 then 1 else -1`, where `-1` is interpreted as `Q-1`.
    ///
    /// If provided, will produce a bundle under `output_moduli` instead of `x.moduli()`
    fn crt_sgn(
        &mut self,
        x: &CrtBundle<Self::Item>,
        accuracy: &str,
        output_moduli: Option<&[u16]>,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        let sign = self.crt_sign(x, accuracy)?;
        output_moduli
            .unwrap_or(&x.moduli())
            .iter()
            .map(|&p| {
                let tt = vec![1, p - 1];
                self.proj(&sign, p, Some(tt))
            })
            .collect::<Result<Vec<Self::Item>, Self::Error>>()
            .map(CrtBundle::new)
    }

    /// Returns 1 if `x < y`.
    fn crt_lt(
        &mut self,
        x: &CrtBundle<Self::Item>,
        y: &CrtBundle<Self::Item>,
        accuracy: &str,
    ) -> Result<Self::Item, Self::Error> {
        let z = self.crt_sub(x, y)?;
        self.crt_sign(&z, accuracy)
    }

    /// Returns 1 if `x >= y`.
    fn crt_geq(
        &mut self,
        x: &CrtBundle<Self::Item>,
        y: &CrtBundle<Self::Item>,
        accuracy: &str,
    ) -> Result<Self::Item, Self::Error> {
        let z = self.crt_lt(x, y, accuracy)?;
        self.negate(&z)
    }

    /// Compute the maximum bundle in `xs`.
    fn crt_max(
        &mut self,
        xs: &[CrtBundle<Self::Item>],
        accuracy: &str,
    ) -> Result<CrtBundle<Self::Item>, Self::Error> {
        if xs.len() < 2 {
            return Err(Self::Error::from(FancyError::InvalidArgNum {
                got: xs.len(),
                needed: 2,
            }));
        }
        xs.iter().skip(1).fold(Ok(xs[0].clone()), |x, y| {
            x.map(|x| {
                let pos = self.crt_lt(&x, y, accuracy)?;
                let neg = self.negate(&pos)?;
                x.wires()
                    .iter()
                    .zip(y.wires().iter())
                    .map(|(x, y)| {
                        let xp = self.mul(x, &neg)?;
                        let yp = self.mul(y, &pos)?;
                        self.add(&xp, &yp)
                    })
                    .collect::<Result<Vec<Self::Item>, Self::Error>>()
                    .map(CrtBundle::new)
            })?
        })
    }
}

/// Compute the ms needed for the number of CRT primes in `x`, with accuracy acc.
///
/// Supported accuracy: ["100%", "99.9%", "99%"]
fn get_ms<W: Clone + HasModulus>(x: &Bundle<W>, accuracy: &str) -> Vec<u16> {
    match accuracy {
        "100%" => match x.moduli().len() {
            3 => vec![2; 5],
            4 => vec![3, 26],
            5 => vec![3, 4, 54],
            6 => vec![5, 5, 5, 60],
            7 => vec![5, 6, 6, 7, 86],
            8 => vec![5, 7, 8, 8, 9, 98],
            9 => vec![5, 5, 7, 7, 7, 7, 7, 76],
            10 => vec![5, 5, 6, 6, 6, 6, 11, 11, 202],
            11 => vec![5, 5, 5, 5, 5, 6, 6, 6, 7, 7, 8, 150],
            n => panic!("unknown exact Ms for {} primes!", n),
        },
        "99.999%" => match x.moduli().len() {
            8 => vec![5, 5, 6, 7, 102],
            9 => vec![5, 5, 6, 7, 114],
            10 => vec![5, 6, 6, 7, 102],
            11 => vec![5, 5, 6, 7, 130],
            n => panic!("unknown 99.999% accurate Ms for {} primes!", n),
        },
        "99.99%" => match x.moduli().len() {
            6 => vec![5, 5, 5, 42],
            7 => vec![4, 5, 6, 88],
            8 => vec![4, 5, 7, 78],
            9 => vec![5, 5, 6, 84],
            10 => vec![4, 5, 6, 112],
            11 => vec![7, 11, 174],
            n => panic!("unknown 99.99% accurate Ms for {} primes!", n),
        },
        "99.9%" => match x.moduli().len() {
            5 => vec![3, 5, 30],
            6 => vec![4, 5, 48],
            7 => vec![4, 5, 60],
            8 => vec![3, 5, 78],
            9 => vec![9, 140],
            10 => vec![7, 190],
            n => panic!("unknown 99.9% accurate Ms for {} primes!", n),
        },
        "99%" => match x.moduli().len() {
            4 => vec![3, 18],
            5 => vec![3, 36],
            6 => vec![3, 40],
            7 => vec![3, 40],
            8 => vec![126],
            9 => vec![138],
            10 => vec![140],
            n => panic!("unknown 99% accurate Ms for {} primes!", n),
        },
        _ => panic!("get_ms: unsupported accuracy {}", accuracy),
    }
}
