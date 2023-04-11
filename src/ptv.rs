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
