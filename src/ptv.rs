use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ViewDeparturesResponse {
    pub departures: Vec<Departure>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Departure {
    pub at_platform: bool,
    pub scheduled_departure_utc: Option<String>,
    pub estimated_departure_utc: Option<String>,
}
