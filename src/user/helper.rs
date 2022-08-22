// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::Write,
    os::unix::{
        io::{IntoRawFd, RawFd},
        process::CommandExt,
    },
    process,
};

use anyhow::{Context, Result};
use memfd::MemfdOptions;
use once_cell::sync::Lazy;

#[expect(
    clippy::expect_used,
    reason = "fails only on unsupported libc versions or OOM"
)]
static HELPER: Lazy<RawFd> = Lazy::new(|| {
    let memfd = MemfdOptions::default()
        .allow_sealing(true)
        .create("helper")
        .expect("could not allocate space for helper binary with memfd_create(2)");
    memfd
        .as_file()
        .write_all(include_bytes!(env!(
            "CARGO_BIN_FILE_SYSTEMD_USER_SLEEP_HELPER"
        )))
        .expect("could not write helper binary data");
    memfd
        .add_seals(&[
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealWrite,
        ])
        .expect("could not seal binary");
    memfd
        .add_seal(memfd::FileSeal::SealSeal)
        .expect("could not seal binary");
    memfd.into_raw_fd()
});

#[derive(Debug)]
pub(super) struct Process {
    child: process::Child,
}

impl Process {
    pub(super) fn new() -> Result<Self> {
        let child = process::Command::new(format!("/proc/self/fd/{}", *HELPER))
            .arg0("systemd-user-sleep-helper")
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .spawn()
            .context("Failed to spawn helper process")?;
        Ok(Self { child })
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(super) async fn wait(mut self) -> Result<process::ExitStatus> {
        tokio::task::spawn_blocking(move || self.child.wait())
            .await
            .context("Failed to read process exit status")?
            .context("Helper exited with non-zero status")
    }
}
