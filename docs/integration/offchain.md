# Off-chain integration surface

What the `offchain` indexer and keeper must target, for the contracts at the
commit that last updated this file. Regenerate the event table from the
source after any change to an event.

## Status of `offchain`

`offchain` (last commit 2026-08-20, `e711ace`) predates the settlement rewrite
(2026-09-15, `3d000cf`/`46abafa`) and everything since. It cannot work
against these contracts as it stands:

- **Indexer.** It handles topics the contracts no longer emit
  (`posinc`, `budgetin`, `budgetout`, `refreg`, `refset`, `refaccr`,
  `refclaim`, `ordplace`, `ordfill`, `ordcancel`). It has no handlers for
  most current topics (see the table below), it imports the deleted
  `oracle-router` binding, and its oracle poller calls the router's
  `get_price` instead of the SEP-40 `lastprice`.
- **Keeper.** It calls `execute_order`, which no longer exists.
- **Data model.** The `referrals` schema has nothing left to index.

Porting it is a project of its own (handlers, keeper flows, DB migrations
under the live app), not a patch.

## Contracts and registry keys

`deployments/<network>.json` / `@win-trader/config` (0.4.0):

| Key | Contract |
|---|---|
| `configManager` | ConfigManager: roles, admin transfer, upgrade timelock |
| `governor` | MarketGovernor: config proposals and timelocks |
| `positionManager` | PositionManager: positions, actions, live config, events for trading |
| `vault` | Vault: cash and `sLP` |
| `requestRouter` | RequestRouter: LP deposit and withdrawal queue |
| `oracleRouter` | The third-party SEP-40 price feed (`lastprice(Asset)`, `decimals()`); read as `Asset::Other(market)` |
| `mockToken`, `oracle` | Local and testnet only |

Index events from all of `configManager`, `governor`, `positionManager`,
`vault`, and `requestRouter`. Decode them with the specs in
`@win-trader/bindings` 0.2.0 (`config-manager`, `market-governor`,
`position-manager`, `vault`, `request-router`); every event's schema is also
in the deployed contract's ABI.

## Keeper entry points

All settlement is permissionless; the caller signs as `keeper` and receives
the reward.

| Work | Call | Notes |
|---|---|---|
| Market open | `PM.settle_market_open(keeper, action_id)` | `NotReady` until the execution delay passes and a price newer than the commit exists |
| Limit open | `PM.settle_limit_open(keeper, action_id)` | `Pending` until the trigger crosses |
| Expired entry | `PM.clean_expired_entry(keeper, action_id)` | Pays the expiry reward |
| Decrease / close | `PM.settle_decrease` / `PM.settle_close(keeper, action_id)` | `RequiresLiquidation` means liquidate first; the action survives |
| Take-profit / stop-loss | `PM.execute_take_profit` / `PM.execute_stop_loss(keeper, position_id)` | `Pending` outside the bound |
| Liquidation | `PM.liquidate_position(keeper, position_id)` | Reverts `PositionHealthy` if not liquidatable |
| ADL | `PM.execute_adl(keeper, position_id)` | Only on an `Adl`/`HardCap` side, profitable positions only |
| Index checkpoint | `PM.update_indices(caller, market)` | No reward |
| LP queue | `RR.resolve_next(executor)` | Pays the reward fixed when the request was made (`LpRequest.reward`) |
| Stuck LP head | `RR.skip_head(caller)` (PAUSER) | Refunds the head in full once it has been resolvable for a day; for a head `resolve_next` cannot settle |
| Due config proposals | `governor.apply_global_config` / `apply_market_config` / `apply_price_feed` | Permissionless once due; lapse one timelock later |
| Storage TTL | Network `ExtendFootprintTTL` operation | No contract entry point |

**Submit with an instruction leeway.** `resolve_next` passed simulation but
failed at submission with `ResourceLimitExceeded` on a local network, and
succeeded with `--instruction-leeway 1000000`. On testnet, every settlement, liquidation, and LP resolution went through with a 2M leeway. Accrual covers more elapsed
time by the time the transaction lands than when it was simulated. Add a
leeway to every settlement, liquidation, and LP resolution, or the LP queue
head can fail repeatedly (THREAT_MODEL D-1).

## Events

| Contract | Topic | Event | Data format | Extra topics | Data fields |
|---|---|---|---|---|---|
| config-manager | `role` | `RoleChange` | vec |  | role, account, is_grant |
| config-manager | `upgtl` | `UpgradeTimelockUpdate` | vec |  | timelock_seconds |
| config-manager | `adminprop` | `AdminProposed` | vec |  | proposer, new_admin |
| config-manager | `admincxl` | `AdminProposalCancelled` | vec |  | canceller |
| market-governor | `cfgprop` | `ConfigurationProposed` | vec |  | header, market, effective_at |
| market-governor | `feedprop` | `PriceFeedProposed` | vec |  | header, price_feed, effective_at |
| market-governor | `cfgcancel` | `ProposalCancelled` | vec |  | header, market, price_feed |
| position-manager | `actcancel` | `ActionCancelled` | map | action_id | header, owner, refund |
| position-manager | `actcommit` | `ActionCommitted` | map | action_id | header, owner, kind, created_at, execute_after, commit_observed_at, escrowed_collateral, expires_at |
| position-manager | `actexpire` | `ActionExpired` | map | action_id | header, owner, keeper, reward, refund |
| position-manager | `actfail` | `ActionFailed` | map | action_id | header, owner, kind, reason, keeper, reward, refund |
| position-manager | `actsettle` | `ActionSettled` | map | action_id | header, owner, kind, position_id, fill_price, fill_observed_at, keeper_reward |
| position-manager | `actsuper` | `ActionSuperseded` | map | action_id | header, position_id, owner, refund |
| position-manager | `baddebt` | `BadDebt` | vec |  | header, position_id, amount |
| position-manager | `borrowchk` | `BorrowCheckpoint` | vec |  | header, elapsed, index_delta, rate_applied, index_after |
| position-manager | `colladd` | `CollateralAdded` | map | position_id | header, owner, amount, stored_collateral |
| position-manager | `fundchk` | `FundingCheckpoint` | map | market | header, segment, payer_side, receiver_backed_delta, lp_backed_delta, receiver_delta, liability_delta, ema_after, elapsed |
| position-manager | `cfgglobal` | `GlobalConfigUpdated` | map |  | header, config |
| position-manager | `mktchk` | `MarketCheckpoint` | map | market | header, receiver_backed_index_long, receiver_backed_index_short, lp_backed_index_long, lp_backed_index_short, receiver_index_long, receiver_index_short, current_payer_side, current_payer_rate, skew_ema, borrow_index, current_borrow_rate, timestamp |
| position-manager | `cfgmarket` | `MarketConfigUpdated` | map | market | header, config |
| position-manager | `mktstatus` | `MarketStatusChanged` | vec |  | header, disabled |
| position-manager | `pause` | `PauseChanged` | vec |  | header, paused |
| position-manager | `paydefer` | `PayoutDeferred` | vec |  | header, owner, amount |
| position-manager | `payclaim` | `PayoutClaimed` | vec |  | header, owner, amount |
| position-manager | `posclose` | `PositionClosed` | map | position_id | header, owner, reason, size, price, raw_pnl, payable_pnl, collateral_payout, bad_debt, unpaid_profit, closing_fee, keeper_reward, keeper_from_lp_backstop, keeper_unpaid, effective_collateral, liquidation_threshold, payout_factor, receiver_funding_paid, lp_funding_paid, borrow_paid, funding_received, loss_collected |
| position-manager | `posdec` | `PositionDecreased` | map | position_id | header, owner, size_removed, price, raw_pnl, payable_pnl, stored_collateral, closing_fee, keeper_reward, receiver_funding_paid, lp_funding_paid, borrow_paid, funding_received, loss_collected |
| position-manager | `posopen` | `PositionOpened` | map | position_id | header, owner, is_long, size, base_exposure, stored_collateral, price, take_profit, stop_loss |
| position-manager | `feedset` | `PriceFeedChanged` | vec |  | header, price_feed |
| position-manager | `protclaim` | `ProtocolClaimed` | vec |  | header, recipient, amount |
| position-manager | `recap` | `Recapitalized` | vec |  | header, contributor, amount |
| position-manager | `revsplit` | `RevenueSplit` | vec |  | header, position_id, source, collected, lp_share, protocol_share |
| position-manager | `riskstate` | `RiskStateChanged` | vec |  | header, is_long, previous_state, next_state, pnl_factor_bps |
| position-manager | `tpsl` | `TpSlUpdated` | map | position_id | header, owner, take_profit, stop_loss |
| request-router | `lpreq` | `LpRequestCreated` | vec |  | request_id, owner, kind, amount, reward, execute_after |
| request-router | `lpres` | `LpRequestResolved` | vec |  | request_id, owner, kind, status, settled_amount, reward |
| request-router | `lpdefer` | `LpPayoutDeferred` | vec |  | owner, amount |
| request-router | `lpskip` | `LpRequestSkipped` | vec |  | request_id, caller |
| request-router | `lpclaim` | `LpPayoutClaimed` | vec |  | owner, amount |
| shared | `upgprp` | `UpgradeProposed` | vec |  | wasm_hash, eta |
| shared | `wired` | `Wired` | vec |  | target, address, caller |
| shared | `migrated` | `Migrated` | vec |  | version, operator |
| shared | `upgcan` | `UpgradeCancelled` | vec |  | caller |
| vault | `lpdep` | `DepositSettled` | map |  | owner, assets, shares, share_supply, vault_nav |
| vault | `lpwd` | `WithdrawalSettled` | map |  | owner, shares, assets, share_supply, vault_nav |
| vault | `cfglp` | `LpConfigUpdated` | map |  | config |
| vault | `pause` | `PauseChanged` | vec |  | paused |

`UpgradeProposed`, `UpgradeCancelled`, and `Migrated` (`shared`) are emitted by
every upgradeable contract; `Wired` by the PositionManager (`set_vault`) and the
vault (`set_request_router`). Most PositionManager events carry an `EventHeader`
(`event_version`, `ledger_timestamp`, `market`, `actor`) as their first field;
`market` is `"vault"` for protocol-wide events.
