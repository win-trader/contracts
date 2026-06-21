CONTRACTS = vault position-manager config-manager oracle oracle-router mock-oracle mock-token
WASM_DIR  = target/wasm32v1-none/release

# Local network
RPC_URL       ?= http://localhost:8000/soroban/rpc
PASSPHRASE    ?= Standalone Network ; February 2017
SOURCE        ?= admin
DEPLOY_CONTRACTS = config-manager oracle-router vault position-manager

.PHONY: build optimize bind test clean up down reset provision-keys provision-keys-testnet deploy deploy-testnet deploy-mainnet deploy-testnet-full upgrade-local upgrade-testnet grant-keepers add-market cex-oracles cex-oracles-testnet local

build:
	cargo build --target wasm32v1-none --release \
		-p vault \
		-p position-manager \
		-p config-manager \
		-p oracle \
		-p oracle-router \
		-p mock-token \
		-p mock-oracle

optimize: build
	@for contract in $(CONTRACTS); do \
		wasm="$(WASM_DIR)/$$(echo $$contract | tr '-' '_').wasm"; \
		echo "Optimizing $$wasm..."; \
		stellar contract optimize --wasm "$$wasm"; \
	done

bind: optimize
	bash scripts/gen-bindings.sh

test:
	cargo test

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

# Push freshly-built WASM to existing on-chain contracts via the OZ
# Upgradeable `upgrade(operator, new_wasm_hash)` entrypoint.
upgrade-local: build
	NETWORK_KEY=local bash scripts/upgrade.sh

upgrade-testnet: build
	NETWORK_KEY=testnet bash scripts/upgrade.sh

grant-keepers:
	bash scripts/grant-keepers.sh

# Incrementally add a market (oracle source + max leverage) to a live
# deployment, no redeploy. Usage: `make add-market SYMBOL=XLMUSD`.
add-market:
	@if [ -z "$(SYMBOL)" ]; then echo "usage: make add-market SYMBOL=XLMUSD"; exit 1; fi
	bash scripts/add-market.sh $(SYMBOL)

# ---- CEX oracle contract instances (on-chain) ----
# Deploys two `oracle` contract instances, generates per-source publisher
# keypairs, binds each as the instance publisher, and registers them as
# primaries on the OracleRouter. The publisher *services* live in the
# win-trader/oracles repo and run against these instances.
cex-oracles:
	bash scripts/deploy-cex-oracles.sh

cex-oracles-testnet:
	NETWORK_KEY=testnet bash scripts/deploy-cex-oracles.sh

deploy-testnet-full: provision-keys-testnet deploy-testnet cex-oracles-testnet
	@echo ""
	@echo "Testnet on-chain environment ready. Runtime artifact: deployments/testnet.json"

# Full local bootstrap (on-chain only): network -> identities -> core
# contracts -> CEX oracle instances. The off-chain stack (indexer, keeper,
# api, frontend, oracle publishers) lives in the offchain / app / oracles
# repos — run them there against this local network + the recorded addresses.
local: up provision-keys deploy cex-oracles
	@echo ""
	@echo "Local on-chain environment ready. Start the off-chain stack from the offchain / app / oracles repos."
