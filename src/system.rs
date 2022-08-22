// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::io::{AsRawFd, RawFd};

use anyhow::{Context, Error, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use log::{debug, warn};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesiredPowerState {
    Run,
    Sleep,
}

impl DesiredPowerState {
    const fn from_prepare_for_sleep(start: bool) -> Self {
        if start {
            Self::Sleep
        } else {
            Self::Run
        }
    }
}

async fn watch_desired_power_state(
    proxy: &crate::api::LoginManagerProxy<'static>,
) -> zbus::Result<watch::Receiver<DesiredPowerState>> {
    let mut stream = proxy.receive_prepare_for_sleep().await?;
    let current = proxy.preparing_for_sleep().await?;
    let (tx, rx) = watch::channel(DesiredPowerState::from_prepare_for_sleep(current));
    let _task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tx.closed() => break,
                maybe_signal = stream.next() => match maybe_signal {
                    None => break,
                    Some(signal) => {
                        #[expect(
                            clippy::unwrap_used,
                            reason = "only fails when the type signature for the signal is wrong"
                        )]
                        let args = signal.args().unwrap();

                        debug!("Sending updated power state: {:?}", *args.start());
                        if tx
                            .send(DesiredPowerState::from_prepare_for_sleep(*args.start()))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    });
    Ok(rx)
}

unsafe fn ensure_cloexec(fd: RawFd) -> zbus::Result<()> {
    let mut flags = libc::fcntl(fd.as_raw_fd(), libc::F_GETFD);
    if flags < 0_i32 {
        return Err(zbus::Error::from(std::io::Error::last_os_error()));
    } else if flags & libc::FD_CLOEXEC != libc::FD_CLOEXEC {
        flags |= libc::FD_CLOEXEC;
        if libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags) < 0_i32 {
            return Err(zbus::Error::from(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

async fn inhibit(
    proxy: &crate::api::LoginManagerProxy<'_>,
) -> zbus::Result<zbus::zvariant::OwnedFd> {
    let fd = proxy
        .inhibit(
            "sleep",
            "systemd-user-sleep",
            "Wait for user sleep target to be reached",
            "delay",
        )
        .await?;
    // SAFETY: We make calls to libc using the raw file descriptor, which could
    // in theory corrupt it (but we make sure not to, of course).
    unsafe {
        ensure_cloexec(fd.as_raw_fd())?;
    }
    Ok(fd)
}

#[async_trait]
pub(crate) trait PowerStateSettler {
    async fn settle(&mut self, state: DesiredPowerState, cancel: CancellationToken) -> Result<()>;
}

pub(crate) async fn manage_sleep_state(
    proxy: &crate::api::LoginManagerProxy<'static>,
    mut settler: impl PowerStateSettler + Send,
) -> Result<()> {
    let mut power_state_watch = watch_desired_power_state(proxy)
        .await
        .context("Failed to watch power state from D-Bus")?;
    let mut inhibitor = None;
    loop {
        let power_state = { *power_state_watch.borrow_and_update() };
        let cancel = CancellationToken::new();
        let (r1, r2) = tokio::join!(
            async {
                match power_state {
                    DesiredPowerState::Run => {
                        if inhibitor.is_none() {
                            inhibitor = match inhibit(proxy).await {
                                Ok(fd) => Some(fd),
                                Err(zbus::Error::MethodError(_, _, _)) => {
                                    warn!("Unable to acquire inhibitor lock, so we might not be able to reach target in time!");
                                    None
                                }
                                Err(otherwise) => {
                                    return Err(Error::new(otherwise).context("Bus error"))
                                }
                            };
                            debug!("Inhibitor acquisition outcome: {:?}", inhibitor);
                        }
                        debug!("Switching to run state");
                        settler.settle(power_state, cancel.clone()).await?;
                    }
                    DesiredPowerState::Sleep => {
                        debug!("Switching to sleep state");
                        settler.settle(power_state, cancel.clone()).await?;
                        debug!("Starting to release inhibitor: {:?}", inhibitor);
                        inhibitor.take().map_or((), drop);
                        debug!("Inhibitor released");
                    }
                }
                Ok(())
            },
            async {
                let result = power_state_watch.changed().await;
                cancel.cancel();
                result.context("Bus error")
            }
        );
        r1?;
        r2?;
    }
}
