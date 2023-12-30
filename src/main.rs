use chrono::{DateTime, Utc};
use lambda_runtime::LambdaEvent;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use settings::Settings;
use tokio::try_join;

mod ptv;
mod settings;
mod stations;

macro_rules! error_resp {
    ($code:expr, $err:expr) => {
        SuccessResponse {
            status_code: $code,
            body: Body::Fail(ErrorBody {
                error_message: $err.to_string(),
            }),
        }
    };
}

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
    pub body: Body,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
enum Body {
    Success(SuccessBody),
    Fail(ErrorBody),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SuccessBody {
    pub to_city_departures: Vec<Departure>,
    pub from_city_departures: Vec<Departure>,
}

#[derive(Debug, Serialize, PartialEq)]
struct Departure {
    pub minutes: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    // pub error_type: String,
    pub error_message: String,
}

type Response = Result<SuccessResponse, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let settings = Settings::new()?;
    // note: we need to makes sure that the handler that returns an error
    // doesn't actually fail, it should just return a response with a 5xx/4xx code
    // https://github.com/awslabs/aws-lambda-rust-runtime/issues/355
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
        .and_then(|params| params.station_name);
    let station = match station {
        Some(station) => station,
        None => return Ok(error_resp!(400, "no station provided in request")),
    };

    let stop_id = stations::get_stop_id_from_name(station.as_str());
    let stop_id = match stop_id {
        Some(stop_id) => stop_id,
        None => {
            return Ok(error_resp!(
                404,
                format!("station: {} is not found", station)
            ))
        }
    };

    let req_to_city = ptv::create_view_departures_request(
        &http_client,
        api_key,
        developer_id,
        platform_one,
        stop_id,
        uri.clone(),
    );
    let req_to_city = match req_to_city {
        Ok(req_to_city) => req_to_city,
        Err(err) => {
            return Ok(error_resp!(
                500,
                format!("could not construct request to city. {}", err)
            ))
        }
    };

    let req_from_city = ptv::create_view_departures_request(
        &http_client,
        api_key,
        developer_id,
        platform_two,
        stop_id,
        uri,
    );
    let req_from_city = match req_from_city {
        Ok(req_from_city) => req_from_city,
        Err(err) => {
            return Ok(error_resp!(
                500,
                format!("could not construct request from city. {}", err)
            ))
        }
    };

    let (to_city_departures, from_city_departures) = match try_join!(
        dispatch_and_parse_request(req_to_city, &http_client),
        dispatch_and_parse_request(req_from_city, &http_client)
    ) {
        Ok((to_city_departures, from_city_departures)) => {
            (to_city_departures, from_city_departures)
        }
        Err(err) => return Ok(err),
    };

    Ok(SuccessResponse {
        status_code: 200,
        body: Body::Success(SuccessBody {
            to_city_departures,
            from_city_departures,
        }),
    })
}

async fn dispatch_and_parse_request(
    request: reqwest::Request,
    client: &Client,
) -> Result<Vec<Departure>, SuccessResponse> {
    let res = client.execute(request).await;
    let res = match res {
        Ok(res) => res,
        Err(err) => return Err(error_resp!(500, format!("failed to send request: {}", err))),
    };

    if !res.status().is_success() {
        return Err(error_resp!(
            424,
            format!(
                "error response received from ptv. code: {}",
                res.status().as_str()
            )
        ));
    }

    let json = res.json::<ptv::ViewDeparturesResponse>().await;

    let json = match json {
        Ok(json) => json,
        Err(err) => {
            return Err(error_resp!(
                500,
                format!("failed to read json response: {}", err)
            ))
        }
    };

    let minutes = get_departure_minutes_from_response(json);
    let minutes = match minutes {
        Ok(minutes) => minutes,
        Err(err) => {
            return Err(error_resp!(
                500,
                format!("could not get departure minutes from request. {}", err)
            ))
        }
    };
    Ok(minutes)
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
        // note: we'll not log any err Results here, in favour of returning the
        // first departure time. in the future we might want to log this?
        .and_then(|d| get_minutes_from_departure(d.clone()).ok());

    if let Some(minutes) = second_time {
        Ok(vec![
            Departure {
                minutes: first_time,
            },
            Departure { minutes },
        ])
    } else {
        Ok(vec![Departure {
            minutes: first_time,
        }])
    }
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
    use http::{self, HeaderMap, HeaderValue};
    use lambda_runtime::{Config, Context};
    use serde_json::{self, json};
    use std::sync::Arc;

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

        let mut headers = HeaderMap::new();
        headers.append("lambda-runtime-deadline-ms", HeaderValue::from_static("1000"));

        let ctx = Context::new("default", Arc::new(Config::default()), &headers.clone()).unwrap();

        let event = LambdaEvent::new(
            Request {
                query_string_parameters: Some(QueryParams {
                    station_name: Some("rushall".to_string()),
                }),
            },
            ctx,
        );

        // act
        let response = handler(event, settings).await.unwrap();

        // assert
        let expected_response = SuccessResponse {
            status_code: 200,
            body: Body::Success(SuccessBody {
                to_city_departures: vec![Departure { minutes: 2 }, Departure { minutes: 4 }],
                from_city_departures: vec![Departure { minutes: 4 }, Departure { minutes: 2 }],
            }),
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
