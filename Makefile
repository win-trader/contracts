CONTRACTS = vault request-router position-manager config-manager market-governor mock-oracle mock-token
WASM_DIR ?= target/wasm32v1-none/release

# Local network
RPC_URL       ?= http://localhost:8000/soroban/rpc
PASSPHRASE    ?= Standalone Network ; February 2017
SOURCE        ?= admin
DEPLOY_CONTRACTS = config-manager vault request-router position-manager


.PHONY: build optimize repro bind check clean up down reset provision-keys provision-keys-testnet deploy deploy-testnet deploy-mainnet deploy-testnet-full upgrade-propose upgrade-execute add-market local

# Panic locations embed absolute source paths. Remap them to fixed prefixes so
# the same commit builds byte-identical WASM on any machine.
CARGO_HOME ?= $(HOME)/.cargo
REPRO_RUSTFLAGS = --remap-path-prefix=$(CARGO_HOME)=/cargo \
	--remap-path-prefix=$(shell rustc --print sysroot)=/rustc \
	--remap-path-prefix=$(CURDIR)=/build

build:
	RUSTFLAGS="$(REPRO_RUSTFLAGS)" cargo build --target wasm32v1-none --release \
		-p vault \
		-p request-router \
		-p position-manager \
		-p config-manager \
		-p market-governor \
		-p mock-token \
		-p mock-oracle

# The stellar CLI bundles its optimizer, so pinning the CLI pins the
# optimized WASM, and so its hash.
STELLAR_CLI_VERSION = 27.0.0

optimize: build
	@stellar --version | grep -q "^stellar $(STELLAR_CLI_VERSION) " || \
		{ echo "stellar CLI $(STELLAR_CLI_VERSION) is required for reproducible optimized WASM"; exit 1; }
	@for contract in $(CONTRACTS); do \
		wasm="$(WASM_DIR)/$$(echo $$contract | tr '-' '_').wasm"; \
		echo "Optimizing $$wasm..."; \
		stellar contract optimize --wasm "$$wasm" 2>/dev/null || exit 1; \
	done
	bash scripts/check-sizes.sh
	@cd $(WASM_DIR) && shasum -a 256 *.optimized.wasm

# Canonical build. The bytes depend on the host (Apple-silicon and x86-64
# Linux hosts order functions differently), so the hashes that are audited and
# deployed come from this pinned Linux image, the same environment CI runs.
# Output goes to target/repro so it never mixes with host builds.
REPRO_IMAGE = rust:1.98.1@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546

repro:
	docker run --rm --platform linux/amd64 -v "$(CURDIR)":/src -w /src $(REPRO_IMAGE) bash -ec '\
		rustup target add wasm32v1-none >/dev/null; \
		apt-get update -qq >/dev/null && apt-get install -y -qq libdbus-1-3 >/dev/null; \
		curl -sSL https://github.com/stellar/stellar-cli/releases/download/v$(STELLAR_CLI_VERSION)/stellar-cli-$(STELLAR_CLI_VERSION)-x86_64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin; \
		export CARGO_TARGET_DIR=target/repro; \
		make optimize WASM_DIR=target/repro/wasm32v1-none/release'

bind: optimize
	bash scripts/gen-bindings.sh

check:
	cargo check --workspace

clean:
	cargo clean

# ---- Local network (Stellar quickstart) ----

up:
	docker compose up -d --wait
	@echo "Local Stellar network ready."

down:
	docker compose down

reset:
	docker compose down -v
	$(MAKE) local

# ---- Identity provisioning ----
# Generates (and on local/testnet, funds) the Stellar identities the protocol
# uses: admin, keeper, binance-oracle, kucoin-oracle. Idempotent — existing
# keys are left in place. Run BEFORE `make deploy` on a fresh environment.
# Secrets land in .env.<network> (mode 600).
provision-keys:
	NETWORK_KEY=local bash scripts/provision-keys.sh

provision-keys-testnet:
	NETWORK_KEY=testnet bash scripts/provision-keys.sh

# ---- Deploy ----
# Network-agnostic — NETWORK_KEY=local goes through deploy.sh just like
# testnet/mainnet. `optimize` is a hard prerequisite: the deploy script
# resolves `.optimized.wasm`.
deploy: optimize
	NETWORK_KEY=local bash scripts/deploy.sh

deploy-testnet: optimize
	NETWORK_KEY=testnet bash scripts/deploy.sh

deploy-mainnet: optimize
	NETWORK_KEY=mainnet bash scripts/deploy.sh

# Timelocked upgrade: propose, wait out the timelock, then execute from the
# same build. NETWORK_KEY defaults to local; UPGRADE_SOURCE names the UPGRADER.
upgrade-propose:
	PHASE=propose bash scripts/upgrade.sh

upgrade-execute:
	PHASE=execute bash scripts/upgrade.sh

# Register a market on a live deployment, no redeploy. The price feed must
# already serve the symbol. Usage: `make add-market SYMBOL=XLMUSD`.
add-market:
	@if [ -z "$(SYMBOL)" ]; then echo "usage: make add-market SYMBOL=XLMUSD"; exit 1; fi
	bash scripts/add-market.sh $(SYMBOL)

# Testnet needs PRICE_FEED_ADDR: the feed is deployed from the oracles repo.
deploy-testnet-full: provision-keys-testnet deploy-testnet
	@echo ""
	@echo "Testnet on-chain environment ready. Runtime artifact: deployments/testnet.json"

# Full local bootstrap (on-chain only): network -> identities -> core
# contracts, priced by the mock oracle. The off-chain stack (indexer, keeper,
# api, frontend, oracle publishers) lives in the offchain / app / oracles
# repos — run them there against this local network + the recorded addresses.
local: up provision-keys deploy
	@echo ""
	@echo "Local on-chain environment ready. Start the off-chain stack from the offchain / app / oracles repos."
