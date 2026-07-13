# Testnet redeploy: exposure PnL and skew carrying fees

This release is intentionally a clean redeploy. It does not include a storage
migration because the protocol is still on testnet and the position/market
wire formats changed.

## Breaking changes

| Removed | Replacement |
|---|---|
| `BorrowRateConfig` | `CarryingFeeConfig` |
| `base_funding_rate_bps` | `max_skew_rate_bps` |
| `update/get_borrow_rate_config` | `update/get_carrying_fee_config` |
| `funding_cut_bps` | removed |
| position entry borrow/funding indices | `borrow_fee_debt`, `skew_fee_debt` |
| signed market funding index | side-specific non-negative skew indices |
| funding pool | removed |
| arithmetic aggregate entry basis | additive base exposure |

`Position.entry_price` and the market average-price fields remain for display
and indexing compatibility, but are derived values. Settlement must use
`base_exposure`.

## Deployment sequence

1. Build and optimize all WASM artifacts from this branch.
2. Deploy a fresh ConfigManager, OracleRouter, PositionManager, and Vault.
3. Wire the Vault into PositionManager and grant operational roles.
4. Confirm `CarryingFeeConfig.max_skew_rate_bps`; the seeded default is 5,000
   bps APR and the hard ceiling is 20,000 bps APR.
5. Configure markets, leverage, and oracle sources.
6. Regenerate clients from the new WASM specs and deploy indexer/keeper/UI code
   that understands exposure and fee-debt fields.
7. Do not import old positions, market aggregates, funding pools, or indices.
8. Run a smoke test covering a long-dominant interval, a short-dominant
   interval, balanced OI, a partial close, and a full close.

The deploy script's ProtocolLimits payload has been updated to remove the old
funding-cut field. Carrying-fee defaults are seeded by the ConfigManager
constructor and may be explicitly overridden after deployment.

## Indexer and UI requirements

- Treat generated bindings as the source of truth for field names.
- Display accrued `borrow_fee` and `skew_fee` as costs; neither is a trader
  receivable.
- Quote `daily_skew` using post-trade OI, but do not include it in the opening
  transaction charge.
- Label derived entry prices as display values. Never reconstruct contract PnL
  from them.
- Reindex from the new deployment ledger; old and new event shapes are not
  wire-compatible.

## Quantitative review

The authoritative formulas, units, rounding order, dust bound, calibration
examples, invariants, and residual risks are in
[`math/pnl-and-skew-fees.md`](math/pnl-and-skew-fees.md). The design decision is
recorded in
[`adr/0008-exposure-pnl-and-skew-carrying-fee.md`](adr/0008-exposure-pnl-and-skew-carrying-fee.md).
