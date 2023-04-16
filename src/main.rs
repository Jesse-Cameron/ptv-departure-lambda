use chrono::{DateTime, Utc};
use lambda_runtime::LambdaEvent;
use serde::{Deserialize, Serialize};
use settings::Settings;
use tokio::try_join;

mod ptv;
mod settings;
mod stations;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    pub query_string_parameters: Option<QueryParams>,
}

#[derive(Deserialize)]
struct QueryParams {
    pub station_name: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SuccessResponse {
    pub status_code: u16,
    pub body: StationDepartures,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct StationDepartures {
    pub to_city_departures: Vec<Departure>,
    pub from_city_departures: Vec<Departure>,
}

#[derive(Debug, Serialize, PartialEq)]
struct Departure {
    pub minutes: i64,
}

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

async fn handler(e: LambdaEvent<Request>, settings: Settings) -> Response {
    let platform_one: u8 = 1;
    let platform_two: u8 = 2;
    let http_client = reqwest::Client::new();
    let developer_id = settings.developer_id;
    let uri = settings.uri;
    let api_key = settings.api_key.as_bytes();
    let station = e
        .payload
        .query_string_parameters
        .and_then(|params| params.station_name)
        .ok_or_else(|| FailureResponse {
            body: "no station provided in request".to_string(),
        })?;

    let stop_id =
        stations::get_stop_id_from_name(station.as_str()).ok_or_else(|| FailureResponse {
            body: format!("station: {} is not supported", station),
        })?;

    let req_to_city = ptv::create_view_departures_request(
        &http_client,
        api_key,
        developer_id,
        platform_one,
        stop_id,
        uri.clone(),
    )
    .map_err(|err| FailureResponse {
        body: format!("could not construct request to city. {}", err.to_string()),
    })?;

    let req_from_city = ptv::create_view_departures_request(
        &http_client,
        api_key,
        developer_id,
        platform_two,
        stop_id,
        uri,
    )
    .map_err(|err| FailureResponse {
        body: format!("could not construct request from city. {}", err.to_string()),
    })?;

    let (res_to_city, res_from_city) = try_join!(
        http_client.execute(req_to_city),
        http_client.execute(req_from_city)
    )
    .map_err(|err| FailureResponse {
        body: format!("did not successfully complete request. {}", err.to_string()),
    })?;

    if !res_to_city.status().is_success() {
        return Err(FailureResponse {
            body: format!(
                "error response received from ptv. code: {}",
                res_to_city.status().as_str(),
            ),
        });
    }

    if !res_from_city.status().is_success() {
        return Err(FailureResponse {
            body: format!(
                "error response received from ptv. code: {}",
                res_from_city.status().as_str(),
            ),
        });
    }

    let (json_to_city, json_from_city) = try_join!(
        res_to_city.json::<ptv::ViewDeparturesResponse>(),
        res_from_city.json::<ptv::ViewDeparturesResponse>()
    )
    .map_err(|err| FailureResponse {
        body: format!("could not read json response. {}", err.to_string()),
    })?;

    let from_city_departures =
        get_departure_minutes_from_response(json_from_city).map_err(|err| FailureResponse {
            body: format!("could not get departure minutes. {}", err.to_string()),
        })?;

    let to_city_departures =
        get_departure_minutes_from_response(json_to_city).map_err(|err| FailureResponse {
            body: format!("could not get departure minutes. {}", err.to_string()),
        })?;

    Ok(SuccessResponse {
        status_code: 200,
        body: StationDepartures {
            to_city_departures,
            from_city_departures,
        },
    })
}

type MaybeDepartureMins = Result<Vec<Departure>, Box<dyn std::error::Error>>;

fn get_departure_minutes_from_response(
    response: ptv::ViewDeparturesResponse,
) -> MaybeDepartureMins {
    let first_departure = response.departures.get(0).ok_or("no departures found")?;
    let first_time = get_minutes_from_departure(first_departure.clone())?;

    let second_time = response
        .departures
        .get(1)
        // note: we'll swallow an error here, in favour of returning the first departure time
        // in the future we might want to log/handle this better?
        .map(|d| get_minutes_from_departure(d.clone()).ok())
        .flatten();

    let mut departures = vec![Departure {
        minutes: first_time,
    }];

    if let Some(minutes) = second_time {
        departures.push(Departure { minutes });
    }

    Ok(departures)
}

fn get_minutes_from_departure(
    departure: ptv::Departure,
) -> Result<i64, Box<dyn std::error::Error>> {
    let arrival_ts = departure
        .estimated_departure_utc
        .or(departure.scheduled_departure_utc)
        .ok_or("could not find timestamps from departure")?;

    let utc = DateTime::parse_from_rfc3339(&arrival_ts)?;
    let now = Utc::now();
    let duration = utc.signed_duration_since(now).num_minutes();
    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use lambda_runtime::Context;
    use serde_json::{self, json};

    #[test]
    fn test_get_minutes_from_response_calc() {
        let tests = vec![
            (190, 3),  // three minutes away
            (179, 2),  // under three minutes away
            (0, 0),    // zero
            (-5, 0),   // five seconds ago
            (-60, -1), // one minute ago
        ];

        for (future_secs, expected_mins) in tests {
            let test_time = Utc::now() + Duration::seconds(future_secs);
            let test_departure = ptv::Departure {
                scheduled_departure_utc: Some(
                    test_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
                ..Default::default()
            };
            let result =
                get_minutes_from_departure(test_departure).expect("failed to find departure times");
            assert_eq!(result, expected_mins)
        }
    }

    #[test]
    fn test_get_minutes_from_response_prioritise_estimated() {
        let test_time_1 = Utc::now() + Duration::seconds(60);
        let test_time_2 = Utc::now() + Duration::seconds(130);
        let test_departure = ptv::Departure {
            scheduled_departure_utc: Some(
                test_time_1.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            estimated_departure_utc: Some(
                test_time_2.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            ..Default::default()
        };

        let result =
            get_minutes_from_departure(test_departure).expect("failed to find departure times");
        assert_eq!(result, 2)
    }

    #[test]
    fn test_get_minutes_for_responses_empty() {
        let test_departure = ptv::Departure {
            estimated_departure_utc: None,
            scheduled_departure_utc: None,
            ..Default::default()
        };

        let result = get_minutes_from_departure(test_departure);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "could not find timestamps from departure"
        );
    }

    #[test]
    fn test_get_departure_minutes_from_response() {
        let test_cases: Vec<(ptv::ViewDeparturesResponse, MaybeDepartureMins)> = vec![
            (
                ptv::ViewDeparturesResponse { departures: vec![] },
                Err("no departures found".into()),
            ),
            (
                ptv::ViewDeparturesResponse {
                    departures: vec![
                        departure_in(Duration::minutes(9)),
                        ptv::Departure {
                            ..Default::default()
                        },
                    ],
                },
                Ok(vec![Departure { minutes: 8 }]),
            ),
            (
                ptv::ViewDeparturesResponse {
                    departures: vec![
                        departure_in(Duration::minutes(3)),
                        departure_in(Duration::minutes(8)),
                    ],
                },
                Ok(vec![Departure { minutes: 2 }, Departure { minutes: 7 }]),
            ),
            (
                ptv::ViewDeparturesResponse {
                    departures: vec![
                        ptv::Departure {
                            scheduled_departure_utc: Some("invalid".to_string()),
                            ..Default::default()
                        },
                        departure_in(Duration::minutes(1)),
                    ],
                },
                Err("input contains invalid characters".into()),
            ),
        ];

        for (response, expected) in test_cases {
            let actual: MaybeDepartureMins = get_departure_minutes_from_response(response.clone());
            match expected {
                Ok(minutes) => assert_eq!(minutes, actual.unwrap()),
                Err(e) => assert_eq!(e.to_string(), actual.unwrap_err().to_string()),
            }
        }
    }

    fn departure_in(time_from_now: Duration) -> ptv::Departure {
        let departure_time = Utc::now() + time_from_now;
        ptv::Departure {
            scheduled_departure_utc: Some(
                departure_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_handler() {
        // arrange
        let departure_time_1 = Utc::now()
            .checked_add_signed(Duration::minutes(2) + Duration::seconds(10))
            .unwrap();
        let departure_time_2 = Utc::now().checked_add_signed(Duration::minutes(5)).unwrap();
        let example_response_body_1 = create_example_response(departure_time_1, departure_time_2);
        let example_response_body_2 = create_example_response(departure_time_2, departure_time_1);

        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("GET", "/v3/departures/route_type/0/stop/1170")
            .match_query(mockito::Matcher::Regex(
                "platform_numbers=1&max_results=2&include_cancelled=false&devid=1&signature=*"
                    .into(),
            ))
            .with_body(example_response_body_1)
            .create_async()
            .await;

        let _m = server
            .mock("GET", "/v3/departures/route_type/0/stop/1170")
            .match_query(mockito::Matcher::Regex(
                "platform_numbers=2&max_results=2&include_cancelled=false&devid=1&signature=*"
                    .into(),
            ))
            .with_body(example_response_body_2)
            .create_async()
            .await;

        let settings = Settings {
            uri: server.url(),
            api_key: "".to_string(),
            developer_id: 1,
        };

        let event = LambdaEvent::new(
            Request {
                query_string_parameters: Some(QueryParams {
                    station_name: Some("rushall".to_string()),
                }),
            },
            Context::default(),
        );

        // act
        let response = handler(event, settings).await.unwrap();

        // assert
        let expected_response = SuccessResponse {
            status_code: 200,
            body: StationDepartures {
                to_city_departures: vec![Departure { minutes: 2 }, Departure { minutes: 4 }],
                from_city_departures: vec![Departure { minutes: 4 }, Departure { minutes: 2 }],
            },
        };
        assert_eq!(response, expected_response);
    }

    fn create_example_response(
        departure_time_1: DateTime<Utc>,
        departure_time_2: DateTime<Utc>,
    ) -> String {
        let j = json!(
            {
                "departures": [
                    {
                        "scheduled_departure_utc": departure_time_1.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                        "estimated_departure_utc": departure_time_1.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                        "at_platform": false
                    },
                    {
                        "scheduled_departure_utc": departure_time_2.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                        "estimated_departure_utc": null,
                        "at_platform": false
                    }
                ],
            }
        );
        j.to_string()
    }
}
