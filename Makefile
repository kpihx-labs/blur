.PHONY: check doctor push setup-interp setup run cargo-check

check:
	@shellcheck -x -P scripts scripts/*.sh
	@./scripts/doctor.sh
	@cargo fmt --all --check
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo test --all-targets --all-features
	@cargo build --release

cargo-check:
	@cargo fmt --all --check
	@cargo clippy --all-targets --all-features -- -D warnings
	@cargo test --all-targets --all-features
	@cargo build --release

doctor:
	@./scripts/doctor.sh

setup-interp:
	@./scripts/setup-web.sh --interp-only

setup:
	@./scripts/setup-web.sh

run:
	@./scripts/run-web.sh

push:
	git push github main
