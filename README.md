## `ptv-departure-lambda`

This project is a rust lambda for requesting the next two departures times for a single station in the PTV network. Currently only the Mernda train line is supported.

Example:

```sh
$ curl "localhost:3000/departures?station_name=clifton_hill"
{
    "toCityDepartures": [{ "minutes": 9 }, { "minutes": 27 }],
    "fromCityDepartures": [{ "minutes": 26 }, { "minutes": 56 }]
}
```

### Local Development

#### Dependencies

To run and test locally, make sure you have installed:

 - [rust](https://www.rust-lang.org/tools/install)
 - [cross](https://github.com/cross-rs/cross)
 - [aws-sam-cli](https://github.com/aws/aws-sam-cli)

#### Setup Config

```sh
cp ./env.json.example ./env.json
```

Update the values in `env.json` with the values provided by [PTV](https://www.ptv.vic.gov.au/footer/data-and-reporting/datasets/ptv-timetable-api/).


#### Run Locally

Once you have your environment setup, you can run the application locally with:

```sh
make build
make invoke
```

If you want to run a local server you can via:

```sh
make build
make start_api
curl "localhost:3000/departures?station_name=clifton_hill"
```
