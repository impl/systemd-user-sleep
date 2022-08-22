// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

use zbus::dbus_proxy;

#[dbus_proxy(
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    interface = "org.freedesktop.login1.Manager"
)]
trait LoginManager {
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;

    #[dbus_proxy(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;

    #[dbus_proxy(property(emits_changed_signal = "false"))]
    fn preparing_for_sleep(&self) -> zbus::Result<bool>;
}

#[dbus_proxy(
    default_service = "org.freedesktop.systemd1",
    interface = "org.freedesktop.systemd1.Job"
)]
trait SystemdJob {
    #[dbus_proxy(property(emits_changed_signal = "const"))]
    fn id(&self) -> zbus::Result<String>;
}

#[dbus_proxy(
    default_service = "org.freedesktop.systemd1",
    interface = "org.freedesktop.systemd1.Unit"
)]
trait SystemdUnit {
    #[dbus_proxy(property(emits_changed_signal = "const"))]
    fn id(&self) -> zbus::Result<String>;
}

#[dbus_proxy(
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    interface = "org.freedesktop.systemd1.Manager"
)]
trait SystemdManager {
    #[dbus_proxy(name = "GetUnitByPID", object = "SystemdUnit")]
    fn get_unit_by_pid(&self, pid: u32);

    #[dbus_proxy(object = "SystemdJob")]
    fn start_transient_unit<'meth>(
        &self,
        name: &str,
        mode: &str,
        properties: &[(&str, zbus::zvariant::Value<'meth>)],
        aux: &[(&str, &[(&str, zbus::zvariant::Value<'meth>)])],
    );

    fn subscribe(&self) -> zbus::Result<()>;

    #[dbus_proxy(signal)]
    fn job_removed(
        &self,
        id: u32,
        job: zbus::zvariant::OwnedObjectPath,
        unit: String,
        result: String,
    );

    #[dbus_proxy(signal)]
    fn unit_removed(&self, id: String, unit: zbus::zvariant::OwnedObjectPath);
}
