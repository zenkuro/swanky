// -*- mode: rust; -*-
//
// This file is part of `scuttlebutt`.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

//! Implementations of AES-128 and AES-256, encryption only, using Intel's
//! AES-NI instructions.
//!
//! Most of this implementation is borrowed and simplified from the `aesni`
//! crate.

pub mod aes128;
pub mod aes256;

#[allow(deprecated)]
pub use aes128::{Aes128, AES, FIXED_KEY_AES128};
pub use aes256::Aes256;
