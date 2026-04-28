//! Network state tracking and diff logic.

use crawl_ipc::events::NetEvent;
use crawl_ipc::types::NetStatus;

#[derive(Clone)]
pub struct NetworkSnapshot {
    pub status: NetStatus,
}

pub struct NetworkState {
    last: Option<NetworkSnapshot>,
}

impl NetworkState {
    pub fn new() -> Self {
        Self { last: None }
    }

    pub fn set_snapshot(&mut self, snapshot: NetworkSnapshot) {
        self.last = Some(snapshot);
    }

    pub fn diff_events(&self, snapshot: &NetworkSnapshot) -> Vec<NetEvent> {
        let mut events = Vec::new();
        let status = &snapshot.status;

        if let Some(prev) = &self.last {
            let prev_status = &prev.status;
            if prev_status.connectivity != status.connectivity {
                events.push(NetEvent::ConnectivityChanged {
                    state: status.connectivity.clone(),
                });
            }
            if prev_status.wifi_enabled != status.wifi_enabled {
                events.push(if status.wifi_enabled {
                    NetEvent::WifiEnabled
                } else {
                    NetEvent::WifiDisabled
                });
            }
            if prev_status.mode != status.mode {
                events.push(NetEvent::ModeChanged {
                    mode: status.mode.clone(),
                });
            }
        } else {
            events.push(NetEvent::ConnectivityChanged {
                state: status.connectivity.clone(),
            });
            events.push(if status.wifi_enabled {
                NetEvent::WifiEnabled
            } else {
                NetEvent::WifiDisabled
            });
            events.push(NetEvent::ModeChanged {
                mode: status.mode.clone(),
            });
        }

        if let Some(ssid) = status.active_ssid.clone() {
            if let Some(iface) = status.interfaces.first().map(|i| i.name.clone()) {
                events.push(NetEvent::Connected {
                    ssid: Some(ssid),
                    iface,
                });
            }
        }

        events
    }
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}
