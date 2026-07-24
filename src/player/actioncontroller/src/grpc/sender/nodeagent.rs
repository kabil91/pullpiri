use common::nodeagent::fromactioncontroller::{
    connect_server, HandleWorkloadRequest, HandleWorkloadResponse,
};
use common::nodeagent::node_agent_connection_client::NodeAgentConnectionClient;
use tonic::{Request, Status};

pub async fn send_workload_handle_request(
    addr: &str,
    request: HandleWorkloadRequest,
) -> Result<HandleWorkloadResponse, Status> {
    let mut client = NodeAgentConnectionClient::connect(connect_server(&addr))
        .await
        .map_err(|e| Status::unavailable(format!("Failed to connect to NodeAgent: {}", e)))?;

    let response = client
        .handle_workload(Request::new(request))
        .await?
        .into_inner();
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_workload_handle_request_unreachable() {
        let req = HandleWorkloadRequest {
            workload_command: 0,
            pod: "test-pod".to_string(),
        };
        let res = send_workload_handle_request("127.0.0.1:59999", req).await;
        assert!(res.is_err());
    }
}
