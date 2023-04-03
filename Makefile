ARCH = x86_64-unknown-linux-musl

.PHONY: build
build:
	cross build --release --target $(ARCH)
	rm -rf ./build
	mkdir -p ./build
	cp -v ./target/$(ARCH)/release/ptv-departure-lambda ./build/bootstrap

.PHONY: start_api
start_api:
	sam local start-api -t ./template.yaml

.PHONY: invoke
invoke:
	sam local invoke -t ./template.yaml

.PHONY: test
test:
	cross test --target $(ARCH)