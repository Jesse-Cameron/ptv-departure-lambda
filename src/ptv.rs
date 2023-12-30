use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ViewDeparturesResponse {
    pub departures: Vec<Departure>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Departure {
    pub at_platform: bool,
    pub scheduled_departure_utc: Option<String>,
    pub estimated_departure_utc: Option<String>,
}

pub fn create_view_departures_request(
    client: &reqwest::Client,
    api_key: &[u8],
    developer_id: u32,
    platform_id: u8,
    stop_id: u32,
    uri: String,
) -> Result<reqwest::Request, Box<dyn std::error::Error>> {
    let route_type: u8 = 0; // 0 = train
    let max_departures: u8 = 2; // we only want the next two departures

    let path = format!(
        "/v3/departures/route_type/{}/stop/{}?platform_numbers={}&max_results={}&include_cancelled=false&devid={}",
        route_type, stop_id, platform_id, max_departures, developer_id
    );

    let signature_bytes = hmac_sha1::hmac_sha1(api_key, path.as_bytes());
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
    fn test_create_view_departures_request() {
        let http_client = reqwest::Client::new();
        let api_key: &[u8] = b"9c132d31-6a30-4cac-8d8b-8a1970834799"; // fake key
        let uri = "http://example.com";
        let res =
            create_view_departures_request(&http_client, api_key, 32, 1, 1170, uri.to_string())
                .unwrap();
        assert_eq!(res.url().as_str(), "http://example.com/v3/departures/route_type/0/stop/1170?platform_numbers=1&max_results=2&include_cancelled=false&devid=32&signature=234004d132ed696e31cbb23f703743df0f2d7ae3")
    }
}
