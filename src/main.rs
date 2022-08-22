// SPDX-FileCopyrightText: 2022 Noah Fontes
//
// SPDX-License-Identifier: Apache-2.0

#![feature(lint_reasons)]
#![warn(
    rust_2018_idioms,
    future_incompatible,
    unused,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    anonymous_parameters,
    deprecated_in_future,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    missing_doc_code_examples,
    private_doc_tests,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::unseparated_literal_suffix,
    clippy::decimal_literal_representation,
    clippy::single_char_lifetime_names,
    clippy::pattern_type_mismatch,
    clippy::fallible_impl_from,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::wildcard_enum_match_arm,
    clippy::deref_by_slicing,
    clippy::default_numeric_fallback,
    clippy::shadow_reuse,
    clippy::clone_on_ref_ptr,
    clippy::todo,
    clippy::string_add,
    clippy::use_debug,
    clippy::future_not_send,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![cfg_attr(not(test), warn(clippy::panic_in_result_fn))]

mod api;
mod system;
mod user;

use std::process;

use anyhow::{Context, Result};
use clap::Parser;
use futures_util::SinkExt;
use log::{error, warn};
use zbus::Connection;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The targets to activate on sleep
    #[clap(short, long, value_parser)]
    activate: Vec<String>,
}

async fn run(args: Args) -> Result<()> {
    loop {
        let mut system_conn = Connection::system()
            .await
            .context("Unable to connect to system bus")?;
        let mut session_conn: Connection = Connection::session()
            .await
            .context("Unable to connect to user session bus")?;

        let login_manager_proxy = api::LoginManagerProxy::new(&system_conn)
            .await
            .context("Unable to connect to LoginManager service on the system bus")?;

        let systemd_manager_proxy = api::SystemdManagerProxy::new(&session_conn)
            .await
            .context("Unable to connect to SystemdManager service on the session bus")?;
        systemd_manager_proxy
            .subscribe()
            .await
            .context("Bus error")?;

        let settler = user::SystemdScopeSettler::new(&systemd_manager_proxy, &args.activate);

        if let Err(e) = system::manage_sleep_state(&login_manager_proxy, settler).await {
            warn!(
                "Sleep management routine interrupted by error: {:#}, retrying...",
                e
            );
        };

        if let Err(e) = session_conn.close().await {
            warn!("Failed to close connection to user session bus: {:#}", e);
        }
        if let Err(e) = system_conn.close().await {
            warn!("Failed to close connection to system bus: {:#}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if systemd_journal_logger::connected_to_journal() {
        #[expect(
            clippy::unwrap_used,
            reason = "fails only when called multiple times in the same program"
        )]
        systemd_journal_logger::init().unwrap();
        log::set_max_level(log::LevelFilter::Debug);
    } else {
        let logger_env = env_logger::Env::new()
            .filter_or("SYSTEMD_USER_SLEEP_LOG", "warn")
            .write_style("SYSTEMD_USER_SLEEP_LOG_STYLE");
        env_logger::Builder::from_env(logger_env).init();
    }

    if let Err(e) = run(args).await {
        error!("We encountered an error: {:#}", e);
        process::exit(1);
    }
}
