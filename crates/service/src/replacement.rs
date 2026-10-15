//! Graceful replacement targets the resolved endpoint and never opens its database until it owns
//! the first pipe instance. A lost shutdown reply is not proof that the request was not received.
use anyhow::Result;

fn shutdown_may_be_in_flight(error: &anyhow::Error) -> bool {
    if let Some(error)=error.downcast_ref::<std::io::Error>() {
        return matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::UnexpectedEof |
            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset);
    }
    error.downcast_ref::<tokio::time::error::Elapsed>().is_some()
        || error.downcast_ref::<serde_json::Error>().is_some_and(|error|error.is_eof())
}

#[cfg(windows)]
pub async fn request_shutdown(endpoint: &pcscope_ipc::Endpoint) -> Result<bool> {
    match pcscope_ipc::call_on(endpoint, pcscope_ipc::Request::Shutdown).await {
        Ok(value) => anyhow::ensure!(value["stopping"]==true,"selected backend did not acknowledge shutdown"),
        Err(error) if error.downcast_ref::<std::io::Error>().is_some_and(|error|error.kind()==std::io::ErrorKind::NotFound) => return Ok(false),
        Err(error) if shutdown_may_be_in_flight(&error) => {},
        Err(error) => return Err(error.context("requesting graceful shutdown of selected backend")),
    }
    Ok(true)
}

pub fn pipe_still_owned(error: &anyhow::Error) -> bool {
    // Windows FIRST_PIPE_INSTANCE reports ACCESS_DENIED while an existing instance owns this
    // exact pipe. Access denial while connecting for Shutdown is rejected above, not ignored.
    error.downcast_ref::<std::io::Error>().is_some_and(|error|matches!(error.raw_os_error(),Some(5 | 231)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_does_not_swallow_access_or_protocol_errors() {
        for kind in [std::io::ErrorKind::PermissionDenied,std::io::ErrorKind::InvalidInput,std::io::ErrorKind::Other] {
            assert!(!shutdown_may_be_in_flight(&std::io::Error::from(kind).into()));
        }
        for kind in [std::io::ErrorKind::NotFound,std::io::ErrorKind::UnexpectedEof,std::io::ErrorKind::BrokenPipe] {
            assert!(shutdown_may_be_in_flight(&std::io::Error::from(kind).into()));
        }
        assert!(shutdown_may_be_in_flight(&serde_json::from_slice::<serde_json::Value>(b"").unwrap_err().into()));
        assert!(!shutdown_may_be_in_flight(&serde_json::from_slice::<serde_json::Value>(b"garbage").unwrap_err().into()));
        assert!(!shutdown_may_be_in_flight(&anyhow::anyhow!("explicit backend rejection")));
        assert!(pipe_still_owned(&std::io::Error::from_raw_os_error(5).into()));
        assert!(!pipe_still_owned(&std::io::Error::from_raw_os_error(123).into()));
    }
}
