# ptv-departure-lambda

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
