// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

#![feature(core_intrinsics, lang_items, rustc_private)]
#![cfg(not(test))]
#![no_std]
#![no_main]

extern crate libc;

#[no_mangle]
unsafe fn main() {
    while libc::getchar() != libc::EOF {}
}

#[lang = "eh_personality"]
#[no_mangle]
pub const extern "C" fn rust_eh_personality() {}

#[lang = "panic_impl"]
#[no_mangle]
pub extern "C" fn rust_begin_panic(_info: &core::panic::PanicInfo) -> ! {
    core::intrinsics::abort()
}
