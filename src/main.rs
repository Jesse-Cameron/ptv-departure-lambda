use lambda_runtime::LambdaEvent;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use settings::Settings;

mod settings;

#[derive(Deserialize)]
struct Request {}

#[derive(Serialize)]
struct SuccessResponse {}

#[derive(Debug, Serialize)]
struct FailureResponse {
    pub body: String,
}

impl std::fmt::Display for FailureResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.body)
    }
}

impl std::error::Error for FailureResponse {}

type Response = Result<SuccessResponse, FailureResponse>;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let settings = Settings::new()?;
    let func = lambda_runtime::service_fn(|e| handler(e, settings.clone()));
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn handler(_e: LambdaEvent<Request>, settings: Settings) -> Response {
    let platform_one: u8 = 1;
    let platform_two: u8 = 2;
    let http_client = reqwest::Client::new();
    let developer_id = settings.developer_id;
    let uri = settings.uri;
    let api_key = settings.api_key.as_bytes();

    let _req_to_city = create_request(
        &http_client,
        api_key,
        developer_id,
        platform_one,
        uri.clone(),
    )
    .map_err(|err| FailureResponse {
        body: format!("could not construct request to city. {}", err.to_string()),
    })?;

    let _req_from_city = create_request(&http_client, api_key, developer_id, platform_two, uri)
        .map_err(|err| FailureResponse {
            body: format!("could not construct request from city. {}", err.to_string()),
        })?;

    Ok(SuccessResponse {})
}

fn create_request(
    client: &Client,
    api_key: &[u8],
    developer_id: u32,
    platform_id: u8,
    uri: String,
) -> Result<reqwest::Request, Box<dyn std::error::Error>> {
    let route_type: u8 = 0; // 0 = train
    let stop_id: u16 = 1170; // TODO: fetch the stop id from a stop name, hardcode to rushall station for now
    let max_departures: u8 = 2; // we only want the next two departures

    let path = format!(
        "/v3/departures/route_type/{}/stop/{}?platform={}&max_results={}&include_cancelled=false&devid={}",
        route_type, stop_id, platform_id, max_departures, developer_id
    );

    let signature_bytes = hmacsha1::hmac_sha1(api_key, path.as_bytes());
    // convert signature bytes into a utf8 string
    let signature = signature_bytes
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join("");

    let res = client
        .get(format!("{}{}", uri, path))
        .query(&[("signature", signature)])
        .build()?;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_successful_request() {
        let http_client = reqwest::Client::new();
        let api_key: &[u8] = b"9c132d31-6a30-4cac-8d8b-8a1970834799"; // fake key
        let uri = "http://example.com";
        let res = create_request(&http_client, api_key, 32, 1, uri.to_string()).unwrap();
        assert_eq!(res.url().as_str(), "http://example.com/v3/departures/route_type/0/stop/1170?platform=1&max_results=2&include_cancelled=false&devid=32&signature=e4526cb4ce2791d438844077d8a1869ce8fe83ca")
    }
}
