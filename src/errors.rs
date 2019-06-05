// -*- mode: rust; -*-
//
// This file is part of `popsicle`.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

/// Errors produced by the private set intersection protocols.
#[derive(Debug)]
pub enum Error {
    /// Coin tossing failed.
    CoinTossError(scuttlebutt::cointoss::Error),
    /// The underlying oblivious PRF failed.
    OprfError(ocelot::Error),
    /// An input/output error occurred.
    IoError(std::io::Error),
    /// The cuckoo hash is full.
    CuckooHashFull,
    /// The provided cuckoo hash set size is invalid.
    InvalidCuckooSetSize(usize),
    /// The provided cuckoo hash parameters are invalid.
    InvalidCuckooParameters {
        /// Number of items.
        nitems: usize,
        /// Number of hashes.
        nhashes: usize,
    },
    /// An error occurred in the PSI protocol.
    PsiProtocolError(String),
    /// An error occurred in the underlying 2PC protocol.
    TwopcError(twopac::Error),
}

impl From<std::io::Error> for Error {
    #[inline]
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<ocelot::Error> for Error {
    #[inline]
    fn from(e: ocelot::Error) -> Error {
        Error::OprfError(e)
    }
}

impl From<scuttlebutt::cointoss::Error> for Error {
    #[inline]
    fn from(e: scuttlebutt::cointoss::Error) -> Error {
        Error::CoinTossError(e)
    }
}

impl From<twopac::Error> for Error {
    #[inline]
    fn from(e: twopac::Error) -> Error {
        Error::TwopcError(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::CoinTossError(e) => write!(f, "coin toss error: {}", e),
            Error::OprfError(e) => write!(f, "oblivious PRF error: {}", e),
            Error::IoError(e) => write!(f, "IO error: {}", e),
            Error::CuckooHashFull => write!(f, "cuckoo hash error: table is full"),
            Error::InvalidCuckooSetSize(n) => {
                write!(f, "cuckoo hash error: invalid set size {}", n)
            }
            Error::InvalidCuckooParameters { nitems, nhashes } => write!(
                f,
                "cuckoo hash error: no parameters set for {} items and {} hashes",
                nitems, nhashes
            ),
            Error::PsiProtocolError(s) => write!(f, "PSI protocol error: {}", s),
            Error::TwopcError(e) => write!(f, "2PC protocol error: {}", e),
        }
    }
}
