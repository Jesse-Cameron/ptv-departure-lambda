ARCH = x86_64-unknown-linux-musl

.PHONY: build
build:
	cross build --release --target $(ARCH)
	rm -rf ./build
	mkdir -p ./build
	cp -v ./target/$(ARCH)/release/ptv-departure-lambda ./build/bootstrap

.PHONY: start_api
start_api:
	sam local start-api -t ./template.yaml -n ./env.json

.PHONY: check
check:
	cross check --target $(ARCH)

.PHONY: lint
lint:
	cargo clippy

.PHONY: invoke
invoke:
	sam local invoke -t ./template.yaml -n ./env.json -e ./example_event.json

.PHONY: test
test:
	cross test --target $(ARCH)
