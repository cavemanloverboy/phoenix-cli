use phoenix::program::{load_with_dispatch, MarketHeader};
use phoenix::quantities::WrapperU64;
use phoenix::state::markets::{FIFOOrderId, FIFORestingOrder, RestingOrder};
use phoenix::state::Side;
use phoenix_sdk::sdk_client::*;
use solana_sdk::clock::Clock;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar;
use std::mem::size_of;

use crate::helpers::print_helpers::get_precision;

pub async fn process_get_open_orders(
    market_pubkey: &Pubkey,
    trader_pubkey: &Pubkey,
    sdk: &SDKClient,
) -> anyhow::Result<()> {
    // Get market account
    let mut market_and_clock = sdk
        .client
        .get_multiple_accounts_with_commitment(
            &[*market_pubkey, sysvar::clock::id()],
            CommitmentConfig::confirmed(),
        )
        .await?
        .value;

    let market_account_data = market_and_clock
        .remove(0)
        .ok_or(anyhow::Error::msg("Market account not found"))?
        .data;

    let clock_account_data = market_and_clock
        .remove(0)
        .ok_or(anyhow::Error::msg("Clock account not found"))?
        .data;

    let clock: Clock = bincode::deserialize(&clock_account_data)
        .map_err(|_| anyhow::Error::msg("Error deserializing clock"))?;

    let (header_bytes, market_bytes) = market_account_data.split_at(size_of::<MarketHeader>());
    let header: &MarketHeader = bytemuck::try_from_bytes(header_bytes)
        .map_err(|e| anyhow::anyhow!("Error getting market header. Error: {:?}", e))?;

    // Derserialize data and load into correct type
    let market = load_with_dispatch(&header.market_size_params, market_bytes)?.inner;

    let trader_index = market
        .get_trader_index(trader_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Trader not found"))?;
    let book_bids = market.get_book(Side::Bid);
    let book_asks = market.get_book(Side::Ask);
    let price_precision: usize =
        get_precision(10_u64.pow(sdk.quote_decimals) / sdk.tick_size_in_quote_atoms_per_base_unit);
    let size_precision: usize = get_precision(sdk.num_base_lots_per_base_unit);

    println!("Open Bids");
    let mut open_bids = vec![];
    open_bids.push(format!(
        "{0: <20} | {1: <20} | {2: <10} | {3: <10} | {4: <15} | {5: <15} ",
        "ID", "Price (ticks)", "Price", "Quantity", "Slots Remaining", "Seconds Remaining"
    ));
    for (order_id, order) in book_bids.iter() {
        if order.trader_index as u32 == trader_index {
            if order.is_expired(clock.slot, clock.unix_timestamp as u64) {
                continue;
            }
            open_bids.push(format_open_orders(
                sdk,
                order_id,
                order,
                price_precision,
                size_precision,
                &clock,
            ));
        }
    }
    open_bids.iter().for_each(|line| println!("{}", line));

    println!();
    println!("Open Asks");
    let mut open_asks = vec![];
    open_asks.push(format!(
        "{0: <20} | {1: <20} | {2: <10} | {3: <10} | {4: <15} | {5: <15} ",
        "ID", "Price (ticks)", "Price", "Quantity", "Slots Remaining", "Seconds Remaining"
    ));
    for (order_id, order) in book_asks.iter() {
        if order.trader_index as u32 == trader_index {
            if order.is_expired(clock.slot, clock.unix_timestamp as u64) {
                continue;
            }
            open_asks.push(format_open_orders(
                sdk,
                order_id,
                order,
                price_precision,
                size_precision,
                &clock,
            ));
        }
    }
    open_asks.iter().for_each(|line| println!("{}", line));

    Ok(())
}

fn format_open_orders(
    sdk: &SDKClient,
    order_id: &FIFOOrderId,
    order: &FIFORestingOrder,
    price_precision: usize,
    size_precision: usize,
    clock: &Clock,
) -> String {
    format!(
        "{0: <20} | {1: <20} | {2: <10} | {3: <10} | {4: <15} | {5: <15} ",
        order_id.order_sequence_number as i64,
        order_id.price_in_ticks,
        format!(
            "{:.1$}",
            sdk.ticks_to_float_price(order_id.price_in_ticks.as_u64()),
            price_precision
        ),
        format!(
            "{:.1$}",
            order.num_base_lots.as_u64() as f64 * sdk.base_lots_to_base_units_multiplier(),
            size_precision,
        ),
        if order.last_valid_slot >= clock.slot {
            (1 + order.last_valid_slot - clock.slot).to_string()
        } else {
            "∞".to_string()
        },
        if order.last_valid_unix_timestamp_in_seconds >= clock.unix_timestamp as u64 {
            (1 + order.last_valid_unix_timestamp_in_seconds - clock.unix_timestamp as u64)
                .to_string()
        } else {
            "∞".to_string()
        }
    )
}
