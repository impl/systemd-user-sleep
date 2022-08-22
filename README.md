<!--
SPDX-FileCopyrightText: 2022 Noah Fontes

SPDX-License-Identifier: Apache-2.0
-->

# systemd-user-sleep

This is a binary that you can use to implement a `sleep.target` equivalent in systemd user sessions. It works by installing a delay inhibitor for system sleep and watching `PrepareForSleep` D-Bus signals from the login manager.
