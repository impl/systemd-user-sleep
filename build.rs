// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

fn main() {
    // https://github.com/rust-lang/cargo/issues/10527
    println!("cargo:rerun-if-changed=helper/");
}
