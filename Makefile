.PHONY: check doctor setup-interp setup run

check:
	@shellcheck -x -P scripts scripts/*.sh
	@./scripts/doctor.sh

doctor:
	@./scripts/doctor.sh

setup-interp:
	@./scripts/setup-web.sh --interp-only

setup:
	@./scripts/setup-web.sh

run:
	@./scripts/run-web.sh
