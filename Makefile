CONTRACTS = vault request-router position-manager config-manager mock-oracle mock-token
WASM_DIR  = target/wasm32v1-none/release

# Local network
RPC_URL       ?= http://localhost:8000/soroban/rpc
PASSPHRASE    ?= Standalone Network ; February 2017
SOURCE        ?= admin
DEPLOY_CONTRACTS = config-manager vault request-router position-manager

.PHONY: build optimize bind check clean up down reset provision-keys provision-keys-testnet deploy deploy-testnet deploy-mainnet deploy-testnet-full upgrade-propose upgrade-execute add-market local

build:
	cargo build --target wasm32v1-none --release \
		-p vault \
		-p request-router \
		-p position-manager \
		-p config-manager \
		-p mock-token \
		-p mock-oracle

optimize: build
	@for contract in $(CONTRACTS); do \
		wasm="$(WASM_DIR)/$$(echo $$contract | tr '-' '_').wasm"; \
		echo "Optimizing $$wasm..."; \
		stellar contract optimize --wasm "$$wasm"; \
	done
	bash scripts/check-sizes.sh

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
