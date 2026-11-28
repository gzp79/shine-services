use regex::bytes::Regex;
use serde::{Deserialize, Serialize};
use shine_core::{serde::default_from_serde, validation::non_zero_duration};
use shine_infra::{
    serde::{anchored, consts, duration},
    sync::RateLimit,
};
use std::time::Duration;
use validator::{Validate, ValidationError};

/// Time limits of a socket. They end the sockets of peers that stopped reading or went silent (a half-open
/// link, a client that is gone without a close): the hub only sees a connection it can still queue to.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
#[validate(schema(function = "SocketTimeouts::validate_idle_longer_than_ping"))]
pub struct SocketTimeouts {
    /// Longest a single frame may take to be written. A peer that does not read blocks the write for good.
    #[serde(default = "duration::seconds::<10>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub write: Duration,
    /// Interval of the server's ws `Ping` frames. A live peer answers each with a `Pong`.
    #[serde(default = "duration::seconds::<30>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub ping_interval: Duration,
    /// Longest the socket may receive nothing, pongs included, before it is ended. Keep it a few ping intervals.
    #[serde(default = "duration::seconds::<90>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub idle: Duration,
    /// Max wait after a client close for the hub to drop the connection, before the send task is aborted.
    #[serde(default = "duration::seconds::<5>", with = "duration::seconds")]
    #[validate(custom(function = "non_zero_duration"))]
    pub send_drain: Duration,
}

impl Default for SocketTimeouts {
    fn default() -> Self {
        default_from_serde()
    }
}

impl SocketTimeouts {
    fn validate_idle_longer_than_ping(socket_timeouts: &SocketTimeouts) -> Result<(), ValidationError> {
        if socket_timeouts.idle <= socket_timeouts.ping_interval {
            let mut err = ValidationError::new("idle_longer_than_ping_interval");
            err.message = Some("idle must be longer than pingInterval".into());
            return Err(err);
        }
        Ok(())
    }
}

/// What one socket may send. Every frame of the client costs a `frames` token, and a subscribe or an unsubscribe
/// costs a `subscribes` token on top of it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SocketLimits {
    #[serde(default = "SocketLimits::default_frames")]
    #[validate(nested)]
    pub frames: RateLimit,
    #[serde(default = "SocketLimits::default_subscribes")]
    #[validate(nested)]
    pub subscribes: RateLimit,
    /// Frames denied in a row before the socket is dropped.
    #[serde(default = "consts::u32::<20>")]
    #[validate(range(min = 1))]
    pub violation_limit: u32,
}

impl Default for SocketLimits {
    fn default() -> Self {
        default_from_serde()
    }
}

impl SocketLimits {
    const fn default_frames() -> RateLimit {
        RateLimit::new(60, 30.0)
    }

    const fn default_subscribes() -> RateLimit {
        RateLimit::new(10, 2.0)
    }
}

/// Raw WebSocket configuration loaded from config files.
#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WsConfig {
    /// Regular expressions for allowed WebSocket target hosts.
    /// Checked against `X-Forwarded-Host`, `Forwarded host=`, then `Host`.
    #[serde(default, with = "anchored")]
    pub allowed_hosts: Vec<Regex>,
    /// Limits that end the sockets of peers that stopped reading or went silent.
    #[serde(default)]
    #[validate(nested)]
    pub timeouts: SocketTimeouts,
    /// What one socket may send.
    #[serde(default)]
    #[validate(nested)]
    pub limits: SocketLimits,
}

impl WsConfig {
    pub fn is_allowed_host(&self, host: &str) -> bool {
        self.allowed_hosts.iter().any(|r| r.is_match(host.as_bytes()))
    }
}
