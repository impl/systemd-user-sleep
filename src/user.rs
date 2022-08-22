// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

mod helper;

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use log::debug;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct SystemdScope<'dbus> {
    proxy: &'dbus crate::api::SystemdManagerProxy<'dbus>,
    waiter: helper::Process,
}

impl<'dbus> SystemdScope<'dbus> {
    async fn new(
        proxy: &'dbus crate::api::SystemdManagerProxy<'dbus>,
        activate: &zbus::zvariant::Value<'dbus>,
    ) -> Result<SystemdScope<'dbus>> {
        let waiter = helper::Process::new()?;
        let process_ids = [waiter.id()];
        let name = format!("systemd-user-sleep-{}.scope", waiter.id());

        let properties = [
            ("Wants", activate.clone()),
            ("After", activate.clone()),
            ("CollectMode", "inactive-or-failed".into()),
            ("PIDs", process_ids[..].into()),
        ];
        debug!(
            "Activating a new scope with the following properties: {:?}",
            properties
        );

        let mut job_removed_stream = proxy.receive_job_removed().await.context("Bus error")?;

        let job = proxy
            .start_transient_unit(&name, "replace", &properties, &[])
            .await
            .context("Bus error")?;

        while let Some(signal) = job_removed_stream.next().await {
            #[expect(
                clippy::unwrap_used,
                reason = "only fails when the type signature for the signal is wrong"
            )]
            let args = signal.args().unwrap();
            if &args.job().as_ref() == job.path() {
                debug!("Start job removed: {}", args.result());
                break;
            }
        }

        Ok(Self { proxy, waiter })
    }

    async fn stop(self) -> Result<()> {
        let mut unit_removed_stream = self
            .proxy
            .receive_unit_removed()
            .await
            .context("Bus error")?;
        let unit = self
            .proxy
            .get_unit_by_pid(self.waiter.id())
            .await
            .context("Bus error")?;
        _ = self.waiter.wait().await?;
        while let Some(signal) = unit_removed_stream.next().await {
            #[expect(
                clippy::unwrap_used,
                reason = "only fails when the type signature for the signal is wrong"
            )]
            let args = signal.args().unwrap();
            if &args.unit().as_ref() == unit.path() {
                debug!("Unit {} removed", unit.path());
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SystemdScopeSettler<'dbus> {
    proxy: &'dbus crate::api::SystemdManagerProxy<'dbus>,
    activate_prop: zbus::zvariant::Value<'dbus>,
    current_scope: Option<SystemdScope<'dbus>>,
}

impl<'dbus> SystemdScopeSettler<'dbus> {
    pub(crate) fn new(
        proxy: &'dbus crate::api::SystemdManagerProxy<'dbus>,
        activate: &[impl AsRef<str>],
    ) -> Self {
        let activate_prop = activate
            .iter()
            .map(|element| String::from(element.as_ref()))
            .collect::<Vec<_>>()
            .into();

        Self {
            proxy,
            activate_prop,
            current_scope: None,
        }
    }
}

#[async_trait]
impl<'dbus> crate::system::PowerStateSettler for SystemdScopeSettler<'dbus> {
    async fn settle(
        &mut self,
        state: crate::system::DesiredPowerState,
        cancel: CancellationToken,
    ) -> Result<()> {
        match state {
            crate::system::DesiredPowerState::Run => {
                debug!(
                    "Tearing down any current scope unit ({:?}) to move to run state",
                    self.current_scope
                );
                if let Some(scope) = self.current_scope.take() {
                    scope.stop().await.context("Failed to stop scope unit")?;
                }
            }
            crate::system::DesiredPowerState::Sleep => {
                debug!(
                    "Ensuring scope unit ({:?}) is running to move to sleep state",
                    self.current_scope
                );
                if self.current_scope.is_none() {
                    tokio::select! {
                        _ = cancel.cancelled() => (),
                        scope = SystemdScope::new(self.proxy, &self.activate_prop) => {
                            self.current_scope = Some(scope.context("Failed to start scope unit")?);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
