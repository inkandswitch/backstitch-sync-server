use std::{collections::HashMap, net::IpAddr, sync::Arc};

use tokio::sync::Mutex;

#[derive(Clone)]
pub struct IpBans {
    ip_bans: Arc<Mutex<HashMap<IpAddr, std::time::Instant>>>,
    ip_failed_attempts: Arc<Mutex<HashMap<IpAddr, i64>>>,
    max_failed_attempts: i64,
    duration: std::time::Duration,
}

impl IpBans {
    pub fn new(duration: std::time::Duration, max_failed_attempts: i64) -> Self {
        IpBans {
            ip_bans: Default::default(),
            ip_failed_attempts: Default::default(),
            duration,
            max_failed_attempts,
        }
    }

    pub async fn ban(&self, ip: &IpAddr) {
        let mut ip_bans = self.ip_bans.lock().await;
        let mut ip_failed_attempts = self.ip_failed_attempts.lock().await;
        let failed_attempts = ip_failed_attempts.get(ip).cloned().unwrap_or(0);
        if failed_attempts >= self.max_failed_attempts {
            tracing::warn!(
                "Client has been banned for {} minutes. IP: {ip}",
                self.duration.as_secs() / 60
            );
            ip_failed_attempts.insert(*ip, 0);
            ip_bans.insert(*ip, std::time::Instant::now());
        } else {
            ip_failed_attempts.insert(*ip, failed_attempts + 1);
        }
    }

    pub async fn unban(&self, ip: &IpAddr) {
        let mut ip_bans = self.ip_bans.lock().await;
        let mut ip_failed_attempts = self.ip_failed_attempts.lock().await;
        // reset failed attempts
        ip_failed_attempts.insert(*ip, 0);
        // remove from banned list
        ip_bans.remove(ip);
    }

    pub async fn is_banned(&self, ip: &IpAddr) -> bool {
        let mut ip_bans = self.ip_bans.lock().await;
        let Some(banned_at) = ip_bans.get(ip) else {
            return false;
        };

        let time_since = std::time::Instant::now() - *banned_at;
        if time_since < self.duration {
            tracing::warn!(
                "Client banned for {} more minutes. IP: {ip}",
                (self.duration - time_since).as_secs() / 60
            );
            true
        } else {
            ip_bans.remove(ip);
            false
        }
    }
}
