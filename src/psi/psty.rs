// -*- mode: rust; -*-
//
// This file is part of `popsicle`.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

//! Implementation of the Pinkas-Tkachenko-Yanai private set intersection
//! protocol (cf. <https://eprint.iacr.org/2019/241>).

use crate::cuckoo::CuckooHash;
use crate::stream;
use crate::utils;
use crate::Error;
use crate::{Receiver as PsiReceiver, Sender as PsiSender};
// use ocelot::oprf::kkrt::Output;
// use ocelot::oprf::{self, Receiver as OprfReceiver, Sender as OprfSender};
// use rand::seq::SliceRandom;
use rand::{CryptoRng, RngCore};
// use scuttlebutt::utils as scutils;
use scuttlebutt::{Block, SemiHonest};
// use sha2::{Digest, Sha256};
// use std::collections::HashSet;
use std::io::{Read, Write};

const NHASHES: usize = 3;

/// Private set intersection sender.
pub struct Sender {}

/// Private set intersection receiver.
pub struct Receiver {}

impl PsiSender for Sender {
    type Msg = Vec<u8>;

    fn init<R: Read + Send, W: Write + Send, RNG: CryptoRng + RngCore>(
        reader: &mut R,
        writer: &mut W,
        rng: &mut RNG,
    ) -> Result<Self, Error> {
        Ok(Sender {})
    }

    fn send<R: Read + Send, W: Write + Send, RNG: CryptoRng + RngCore>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        inputs: &[Self::Msg],
        mut rng: &mut RNG,
    ) -> Result<(), Error> {
        let n = inputs.len();

        let key = rand::random::<Block>();
        let hashed_inputs = utils::compress_and_hash_inputs(inputs, key);
        let cuckoo = CuckooHash::new(&hashed_inputs, NHASHES)?;

        let nbins = cuckoo.nbins;
        assert_eq!(cuckoo.stashsize, 0);

        // Send cuckoo hash info to receiver.
        key.write(writer)?;
        stream::write_usize(writer, nbins)?;
        writer.flush()?;

        // Set up inputs to use `x || i` or `x` depending on whether the input
        // is in a bin or the stash.
        let table = cuckoo
            .items()
            .map(|opt_item| {
                opt_item
                    .as_ref()
                    .map_or(Block::default(), |item| item.entry)
            })
            .collect::<Vec<Block>>();

        unimplemented!()
    }
}

impl PsiReceiver for Receiver {
    type Msg = Vec<u8>;

    fn init<R: Read + Send, W: Write + Send, RNG: CryptoRng + RngCore>(
        reader: &mut R,
        writer: &mut W,
        rng: &mut RNG,
    ) -> Result<Self, Error> {
        Ok(Receiver {})
    }

    fn receive<R, W, RNG>(
        &mut self,
        reader: &mut R,
        writer: &mut W,
        inputs: &[Self::Msg],
        rng: &mut RNG,
    ) -> Result<Vec<Self::Msg>, Error>
    where
        R: Read + Send,
        W: Write + Send,
        RNG: CryptoRng + RngCore,
    {
        // receive cuckoo hash info from sender
        let key = Block::read(reader)?;
        let nbins = stream::read_usize(reader)?;

        let inputs = utils::compress_and_hash_inputs(inputs, key);

        // map inputs to table using all hash functions
        let mut table = vec![Vec::new(); nbins];

        for h in 0..NHASHES {
            for &x in &inputs {
                let bin = CuckooHash::bin(x, h, nbins);
                table[bin].push(x);
            }
        }

        unimplemented!()
    }
}

impl SemiHonest for Sender {}
impl SemiHonest for Receiver {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::rand_vec_vec;
    use scuttlebutt::AesRng;
    use std::io::{BufReader, BufWriter};
    use std::os::unix::net::UnixStream;
    use std::time::SystemTime;

    const SIZE: usize = 16;
    const NTIMES: usize = 1 << 10;

    #[test]
    fn test_psi() {
        let (sender, receiver) = UnixStream::pair().unwrap();
        let sender_inputs = rand_vec_vec(NTIMES, SIZE);
        let receiver_inputs = sender_inputs.clone();

        let handle = std::thread::spawn(move || {
            let mut rng = AesRng::new();
            let mut reader = BufReader::new(sender.try_clone().unwrap());
            let mut writer = BufWriter::new(sender);

            let start = SystemTime::now();
            let mut psi = Sender::init(&mut reader, &mut writer, &mut rng).unwrap();
            println!(
                "Sender init time: {} ms",
                start.elapsed().unwrap().as_millis()
            );

            let start = SystemTime::now();
            psi.send(&mut reader, &mut writer, &sender_inputs, &mut rng)
                .unwrap();
            println!(
                "[{}] Send time: {} ms",
                NTIMES,
                start.elapsed().unwrap().as_millis()
            );
        });

        let mut rng = AesRng::new();
        let mut reader = BufReader::new(receiver.try_clone().unwrap());
        let mut writer = BufWriter::new(receiver);

        let start = SystemTime::now();
        let mut psi = Receiver::init(&mut reader, &mut writer, &mut rng).unwrap();
        println!(
            "Receiver init time: {} ms",
            start.elapsed().unwrap().as_millis()
        );

        let start = SystemTime::now();
        let intersection = psi
            .receive(&mut reader, &mut writer, &receiver_inputs, &mut rng)
            .unwrap();
        println!(
            "[{}] Receiver time: {} ms",
            NTIMES,
            start.elapsed().unwrap().as_millis()
        );

        handle.join().unwrap();
        assert_eq!(intersection.len(), NTIMES);
    }
}
