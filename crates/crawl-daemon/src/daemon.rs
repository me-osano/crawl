//! Daemon orchestrator for crawl.
//! Manages lifecycle, service startup/shutdown, and coordination.

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::event_bus::EventBus;
use crate::services::ServiceRegistry;
use crate::state::AppState;
use crate::dispatcher::Dispatcher;

/// Main daemon orchestrator.
pub struct Daemon {
    config: Config,
    event_bus: EventBus,
    state: Arc<AppState>,
    dispatcher: Dispatcher,
}

impl Daemon {
    /// Create a new daemon instance.
    pub fn new() -> Result<Self> {
        let config = crate::config::load()?;
        info!(
            config_path = ?config.config_path,
            "Config loaded"
        );

        let event_bus = EventBus::new(100);
        let state = Arc::new(AppState::new(config.clone(), event_bus.clone()));
        let dispatcher = Dispatcher::new();

        Ok(Self {
            config,
            event_bus: event_bus.clone(),
            state,
            dispatcher,
        })
    }

    /// Register all services.
    pub fn register_services(&mut self) {
        use crate::services::audio::AudioService;
        use crate::services::network::NetworkService;
        use crate::services::display::DisplayService;
        use crate::services::sysinfo::SysinfoService;
        use crate::services::bluetooth::BluetoothService;
        use crate::services::sysmon::SysmonService;
        use crate::services::proc::ProcService;

        let registry = self.dispatcher.registry();

        // Audio service
        let audio_svc = AudioService::new(self.state.clone());
        registry.register(std::sync::Arc::new(audio_svc));

        // Network service
        let network_svc = NetworkService::new(self.state.clone());
        registry.register(std::sync::Arc::new(network_svc));

        // Display service
        let display_svc = DisplayService::new(self.state.clone());
        registry.register(std::sync::Arc::new(display_svc));

        // Sysinfo service
        let sysinfo_svc = SysinfoService::new(self.state.clone());
        registry.register(std::sync::Arc::new(sysinfo_svc));

        // Bluetooth service
        let bt_svc = BluetoothService::new(self.state.clone());
        registry.register(std::sync::Arc::new(bt_svc));

        // Sysmon service
        let sysmon_svc = SysmonService::new(self.state.clone());
        registry.register(std::sync::Arc::new(sysmon_svc));

        // Proc service
        let proc_svc = ProcService::new(self.state.clone());
        registry.register(std::sync::Arc::new(proc_svc));
    }

    /// Start the daemon (blocks until shutdown signal).
    pub async fn run(&mut self) -> Result<()> {
        info!(
            socket_path = %self.config.daemon.socket_path,
            services = self.dispatcher.services.services.len(),
            "crawl-daemon starting"
        );

        // Start all services
        let _ = self.dispatcher.services.start_all().await;

        // Start periodic health checks
        self.spawn_health_monitor();

        // Start IPC server
        let socket_path = self.config.daemon.socket_path.clone();
        let mut ipc_server = crawl_ipc::IpcServer::new(
            socket_path.into(),
            self.event_bus.sender(),
        );

        let dispatcher = Arc::new(self.dispatcher.clone());
        let dispatch = move |method: String, params: serde_json::Value, id: Option<serde_json::Value>| {
            let d = dispatcher.clone();
            Box::pin(async move {
                d.dispatch(method, params, id).await
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = crawl_ipc::Response> + Send>>
        };
        ipc_server.set_dispatcher(Arc::new(dispatch) as crawl_ipc::RequestDispatcher);

        tokio::spawn(async move {
            if let Err(e) = ipc_server.run().await {
                error!("IPC server error: {}", e);
            }
        });

        info!(
            socket_path = %self.config.daemon.socket_path,
            "crawl-daemon running"
        );

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("crawl-daemon shutting down");

        // Cancel health monitor
        // (tokio::spawn doesn't provide a handle, but the task will end when daemon ends)

        // Graceful shutdown
        self.graceful_shutdown().await?;

        Ok(())
    }

    /// Spawn periodic health check task.
    fn spawn_health_monitor(&self) {
        let services = Arc::new(self.dispatcher.services.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let health = services.health_check();
                if !health.values().all(|&v| v) {
                    warn!(?health, "Unhealthy services detected");
                }
            }
        });
    }

    /// Graceful shutdown: clean up resources.
    async fn graceful_shutdown(&self) -> Result<()> {
        // Clean up socket file
        let socket_path = &self.config.daemon.socket_path;
        if std::path::Path::new(socket_path).exists() {
            if let Err(e) = std::fs::remove_file(socket_path) {
                warn!("Failed to remove socket file {}: {}", socket_path, e);
            } else {
                info!("Removed socket file: {}", socket_path);
            }
        }

        // Stop all services
        self.dispatcher.services.stop_all().await?;

        info!("crawl-daemon shutdown complete");
        Ok(())
    }

    #[allow(dead_code)]
    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    #[allow(dead_code)]
    /// Get a reference to the service registry.
    pub fn services(&self) -> &ServiceRegistry {
        &self.dispatcher.services
    }
}
