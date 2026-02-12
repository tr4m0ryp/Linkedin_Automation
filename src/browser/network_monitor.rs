use crate::error::{LinkedInError, Result};
use super::types::{NetworkRequest, NetworkResponse};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, debug, error};
use chrono::Utc;

pub struct NetworkMonitor {
    requests: Arc<RwLock<HashMap<String, NetworkRequest>>>,
    responses: Arc<RwLock<HashMap<String, NetworkResponse>>>,
    monitor_task: Option<JoinHandle<()>>,
    debug_port: u16,
    filter_patterns: Vec<String>,
}

impl NetworkMonitor {
    pub async fn new(debug_port: u16) -> Result<Self> {
        let filter_patterns = vec![
            "/voyager/api/".to_string(),
            "/voyagerapi/".to_string(),
            "/growth/".to_string(),
            "/normInvitations".to_string(),
            "/flagship-web/rsc-action/".to_string(),
            "addaAddConnection".to_string(),
        ];

        Ok(Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            responses: Arc::new(RwLock::new(HashMap::new())),
            monitor_task: None,
            debug_port,
            filter_patterns,
        })
    }

    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!("Starting network monitoring");

        let client = Client::new();
        let cdp_url = format!("http://localhost:{}/json", self.debug_port);

        debug!("Fetching CDP target from: {}", cdp_url);
        let response = client.get(&cdp_url).send().await
            .map_err(|e| LinkedInError::BrowserError(format!("CDP connection failed: {}", e)))?;

        let targets: Vec<Value> = response.json().await
            .map_err(|e| LinkedInError::BrowserError(format!("Failed to parse CDP targets: {}", e)))?;

        let target = targets.iter()
            .find(|t| t["type"] == "page")
            .ok_or_else(|| LinkedInError::BrowserError("No page target found".to_string()))?;

        let ws_url = target["webSocketDebuggerUrl"].as_str()
            .ok_or_else(|| LinkedInError::BrowserError("No WebSocket URL found".to_string()))?
            .to_string();

        info!("Connecting to CDP WebSocket: {}", ws_url);

        let requests = self.requests.clone();
        let responses = self.responses.clone();
        let filter_patterns = self.filter_patterns.clone();

        let task = tokio::spawn(async move {
            if let Err(e) = Self::monitor_loop(ws_url, requests, responses, filter_patterns).await {
                error!("Network monitor error: {}", e);
            }
        });

        self.monitor_task = Some(task);
        info!("Network monitoring started");

        Ok(())
    }

    async fn monitor_loop(
        ws_url: String,
        requests: Arc<RwLock<HashMap<String, NetworkRequest>>>,
        responses: Arc<RwLock<HashMap<String, NetworkResponse>>>,
        filter_patterns: Vec<String>,
    ) -> Result<()> {
        use tokio_tungstenite::{connect_async, tungstenite::Message};
        use futures_util::StreamExt;

        let (ws_stream, _) = connect_async(&ws_url).await
            .map_err(|e| LinkedInError::BrowserError(format!("WebSocket connection failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        let enable_network = serde_json::json!({
            "id": 1,
            "method": "Network.enable",
            "params": {}
        });

        use futures_util::SinkExt;
        write.send(Message::Text(enable_network.to_string())).await
            .map_err(|e| LinkedInError::BrowserError(format!("Failed to enable network: {}", e)))?;

        info!("CDP Network domain enabled");

        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(event) = serde_json::from_str::<Value>(&text) {
                    Self::handle_event(event, &requests, &responses, &filter_patterns).await;
                }
            }
        }

        Ok(())
    }

    async fn handle_event(
        event: Value,
        requests: &Arc<RwLock<HashMap<String, NetworkRequest>>>,
        responses: &Arc<RwLock<HashMap<String, NetworkResponse>>>,
        filter_patterns: &[String],
    ) {
        let method = event["method"].as_str().unwrap_or("");

        match method {
            "Network.requestWillBeSent" => {
                if let Some(params) = event["params"].as_object() {
                    let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                    let url = params["request"]["url"].as_str().unwrap_or("").to_string();

                    if Self::should_log(&url, filter_patterns) {
                        let method_type = params["request"]["method"].as_str().unwrap_or("GET").to_string();
                        let headers = Self::extract_headers(&params["request"]["headers"]);
                        let post_data = params["request"]["postData"].as_str().map(|s| s.to_string());

                        let req = NetworkRequest {
                            request_id: request_id.clone(),
                            url: url.clone(),
                            method: method_type.clone(),
                            headers,
                            post_data: post_data.clone(),
                            timestamp: Utc::now().timestamp(),
                        };

                        info!("API REQUEST: {} {}", method_type, url);
                        if let Some(data) = &post_data {
                            debug!("POST data: {}", data);
                        }

                        requests.write().await.insert(request_id, req);
                    }
                }
            }
            "Network.responseReceived" => {
                if let Some(params) = event["params"].as_object() {
                    let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                    let url = params["response"]["url"].as_str().unwrap_or("").to_string();

                    if Self::should_log(&url, filter_patterns) {
                        let status = params["response"]["status"].as_u64().unwrap_or(0) as u16;
                        let headers = Self::extract_headers(&params["response"]["headers"]);

                        let resp = NetworkResponse {
                            request_id: request_id.clone(),
                            url: url.clone(),
                            status,
                            headers,
                            body: None,
                            timestamp: Utc::now().timestamp(),
                        };

                        info!("API RESPONSE: {} - {}", status, url);

                        responses.write().await.insert(request_id, resp);
                    }
                }
            }
            _ => {}
        }
    }

    fn should_log(url: &str, filter_patterns: &[String]) -> bool {
        filter_patterns.iter().any(|pattern| url.contains(pattern))
    }

    fn extract_headers(headers_value: &Value) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        if let Some(obj) = headers_value.as_object() {
            for (key, value) in obj {
                if let Some(val_str) = value.as_str() {
                    headers.insert(key.clone(), val_str.to_string());
                }
            }
        }
        headers
    }

    pub async fn get_requests(&self) -> HashMap<String, NetworkRequest> {
        self.requests.read().await.clone()
    }

    pub async fn get_responses(&self) -> HashMap<String, NetworkResponse> {
        self.responses.read().await.clone()
    }

    pub async fn export_to_file(&self, filepath: &str) -> Result<()> {
        let requests = self.get_requests().await;
        let responses = self.get_responses().await;

        let export_data = serde_json::json!({
            "requests": requests,
            "responses": responses,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json_str = serde_json::to_string_pretty(&export_data)
            .map_err(|e| LinkedInError::BrowserError(format!("Failed to serialize JSON: {}", e)))?;
        std::fs::write(filepath, json_str)
            .map_err(|e| LinkedInError::BrowserError(format!("Failed to write file: {}", e)))?;

        info!("Network traffic exported to: {}", filepath);
        Ok(())
    }

    pub async fn stop(self) -> Result<()> {
        if let Some(task) = self.monitor_task {
            task.abort();
        }
        info!("Network monitor stopped");
        Ok(())
    }
}
