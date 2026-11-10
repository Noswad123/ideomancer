//! Qualified public SDK operation rules. No tenant or application wrapper semantics.
pub fn supported_root(qualified: &str) -> bool {
    matches!(
        qualified.split('.').next(),
        Some("httpx" | "requests" | "aiohttp" | "benchling_sdk" | "benchling_api_client")
    )
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_alphabetic() || (i > 0 && c.is_numeric()))
}

pub fn network_operation(qualified: &str, suffix: &str) -> Option<(String, String, &'static str)> {
    let (client, operation) = if suffix.is_empty() {
        let (client, operation) = qualified.rsplit_once('.')?;
        (client, operation)
    } else {
        (qualified, suffix)
    };
    let root = client.split('.').next()?;
    if matches!(root, "httpx" | "requests" | "aiohttp") {
        let known_client = matches!(
            client,
            "httpx"
                | "httpx.Client"
                | "httpx.AsyncClient"
                | "httpx._client.Client"
                | "httpx._client.AsyncClient"
                | "requests"
                | "requests.api"
                | "requests.Session"
                | "requests.sessions.Session"
                | "aiohttp"
                | "aiohttp.ClientSession"
                | "aiohttp.client.ClientSession"
        );
        return (known_client
            && matches!(
                operation,
                "get"
                    | "post"
                    | "put"
                    | "patch"
                    | "delete"
                    | "head"
                    | "options"
                    | "trace"
                    | "request"
                    | "send"
            ))
        .then(|| {
            (
                client.into(),
                operation.into(),
                "HTTP client/import boundary candidate",
            )
        });
    }
    if matches!(root, "benchling_sdk" | "benchling_api_client") {
        let parts: Vec<_> = operation.split('.').collect();
        let service_client = client.split('.').any(|p| p == "services");
        let benchling_client = matches!(
            client,
            "benchling_sdk.Benchling"
                | "benchling_sdk.benchling.Benchling"
                | "benchling_api_client.Benchling"
        );
        let helper = |p: &str| {
            matches!(
                p,
                "auth"
                    | "models"
                    | "model"
                    | "schemas"
                    | "schema"
                    | "helpers"
                    | "utils"
                    | "util"
                    | "api_client"
                    | "configuration"
                    | "to_dict"
                    | "from_dict"
                    | "to_json"
                    | "from_json"
                    | "model_dump"
                    | "model_validate"
                    | "parse_obj"
                    | "serialize"
                    | "deserialize"
            )
        };
        if (benchling_client && parts.len() >= 2 || service_client && !parts.is_empty())
            && !client.split('.').any(helper)
            && parts.iter().all(|p| {
                !helper(p) && p.starts_with(|c: char| c.is_ascii_lowercase()) && identifier(p)
            })
        {
            return Some((
                client.into(),
                operation.into(),
                "Benchling SDK boundary candidate",
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_sdk_rules_require_qualified_identity_and_exclude_helpers() {
        assert!(
            network_operation("benchling_sdk.benchling.Benchling", "containers.list").is_some()
        );
        assert!(network_operation(
            "benchling_sdk.services.container_service.ContainerService",
            "list"
        )
        .is_some());
        for (client, operation) in [
            ("application.DynamicClientWrapper", "containers.list"),
            ("application.Benchling", "containers.list"),
            ("benchling_sdk.benchling.Benchling", "models.parse_obj"),
            ("benchling_sdk.auth.Client", "request"),
            ("benchling_sdk.benchling.Benchling", "list"),
        ] {
            assert!(network_operation(client, operation).is_none());
        }
    }

    #[test]
    fn http_clients_preserve_their_own_operation_allowlist() {
        for client in [
            "requests",
            "requests.Session",
            "httpx.AsyncClient",
            "aiohttp.ClientSession",
        ] {
            assert!(supported_root(client));
            assert!(network_operation(client, "get").is_some());
            assert!(network_operation(client, "model_dump").is_none());
            assert!(network_operation(client, "resources.get").is_none());
        }
    }
}
