mod xcm_balances;
mod xcm_generic;

use frame_support::weights::Weight;
use xcm_balances::WeightInfo as XcmBalancesWeight;

use crate::Runtime;
use sp_std::prelude::*;
use xcm::{latest::prelude::*, DoubleEncoded};
use xcm_generic::WeightInfo as XcmGeneric;

pub enum AssetTypes {
	Balances,
	Unknown,
}

impl From<&MultiAsset> for AssetTypes {
	fn from(asset: &MultiAsset) -> Self {
		match asset {
			MultiAsset { id: Concrete(MultiLocation { parents: 0, interior: Here }), .. } =>
				AssetTypes::Balances,
			_ => AssetTypes::Unknown,
		}
	}
}

// TODO from Shawn: I dont like this
trait WeighMultiAssets {
	fn weigh_multi_assets(&self, balances_weight: Weight) -> Weight;
}

// TODO wild case
impl WeighMultiAssets for MultiAssetFilter {
	fn weigh_multi_assets(&self, balances_weight: Weight) -> Weight {
		match self {
			Self::Definite(assets) => assets
				.inner()
				.into_iter()
				.map(|m| <AssetTypes as From<&MultiAsset>>::from(m))
				.map(|t| match t {
					AssetTypes::Balances => balances_weight,
					AssetTypes::Unknown => Weight::MAX,
				})
				.fold(0, |acc, x| acc.saturating_add(x)),
			_ => Weight::MAX,
		}
	}
}

impl WeighMultiAssets for MultiAssets {
	fn weigh_multi_assets(&self, balances_weight: Weight) -> Weight {
		self.inner()
			.into_iter()
			.map(|m| <AssetTypes as From<&MultiAsset>>::from(m))
			.map(|t| match t {
				AssetTypes::Balances => balances_weight,
				AssetTypes::Unknown => Weight::MAX,
			})
			.fold(0, |acc, x| acc.saturating_add(x))
	}
}

pub struct WestendXcmWeight;
impl XcmWeightInfo<()> for WestendXcmWeight {
	fn withdraw_asset(assets: &MultiAssets) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::withdraw_asset())
	}
	fn reserve_asset_deposited(assets: &MultiAssets) -> Weight {
		assets.weigh_multi_assets(XcmGeneric::<Runtime>::reserve_asset_deposited())
	}
	// TODO none of these need effects
	fn receive_teleported_asset(assets: &MultiAssets) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::receive_teleported_asset())
	}
	fn query_response(_query_id: &u64, _response: &Response, max_weight: &u64) -> Weight {
		*max_weight
	}
	fn transfer_asset(assets: &MultiAssets, _dest: &MultiLocation) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::transfer_asset())
	}
	fn transfer_reserve_asset(
		assets: &MultiAssets,
		_dest: &MultiLocation,
		_xcm: &Xcm<()>,
	) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::transfer_reserve_asset())
	}
	fn transact(
		_origin_type: &OriginKind,
		_require_weight_at_most: &u64,
		_call: &DoubleEncoded<()>,
	) -> Weight {
		XcmGeneric::<Runtime>::transact()
	}
	fn hrmp_new_channel_open_request(
		_sender: &u32,
		_max_message_size: &u32,
		_max_capacity: &u32,
	) -> Weight {
		XcmGeneric::<Runtime>::hrmp_new_channel_open_request()
	}
	fn hrmp_channel_accepted(_recipient: &u32) -> Weight {
		XcmGeneric::<Runtime>::hrmp_channel_accepted()
	}
	fn hrmp_channel_closing(_initiator: &u32, _sender: &u32, _recipient: &u32) -> Weight {
		XcmGeneric::<Runtime>::hrmp_channel_closing()
	}
	fn clear_origin() -> Weight {
		XcmGeneric::<Runtime>::clear_origin()
	}
	fn descend_origin(who: &InteriorMultiLocation) -> Weight {
		XcmGeneric::<Runtime>::descend_origin(who)
	}
	fn report_error(
		_query_id: &QueryId,
		_dest: &MultiLocation,
		max_response_weight: &u64,
	) -> Weight {
		*max_response_weight
	}
	fn relayed_from(_who: &Junctions, _message: &Box<Xcm<()>>) -> Weight {
		XcmGeneric::<Runtime>::relayed_from()
	}

	fn deposit_asset(
		assets: &MultiAssetFilter,
		_max_assets: &u32, // TODO use max assets?
		_dest: &MultiLocation,
	) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::deposit_asset())
	}
	fn deposit_reserve_asset(
		assets: &MultiAssetFilter,
		_max_assets: &u32, // TODO use max assets?
		_dest: &MultiLocation,
		_xcm: &Xcm<()>,
	) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::deposit_reserve_asset())
	}
	fn exchange_asset(_give: &MultiAssetFilter, _receive: &MultiAssets) -> Weight {
		Weight::MAX // todo fix
	}
	fn initiate_reserve_withdraw(
		assets: &MultiAssetFilter,
		_reserve: &MultiLocation,
		_xcm: &Xcm<()>,
	) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::initiate_reserve_withdraw())
	}
	fn initiate_teleport(
		assets: &MultiAssetFilter,
		_dest: &MultiLocation,
		_xcm: &Xcm<()>,
	) -> Weight {
		assets.weigh_multi_assets(XcmBalancesWeight::<Runtime>::initiate_teleport())
	}
	fn query_holding(
		_query_id: &u64,
		_dest: &MultiLocation,
		_assets: &MultiAssetFilter,
		_max_response_weight: &u64,
	) -> Weight {
		XcmGeneric::<Runtime>::query_holding()
	}
	fn buy_execution(_fees: &MultiAsset, _weight_limit: &WeightLimit) -> Weight {
		XcmGeneric::<Runtime>::buy_execution()
	}
	fn refund_surplus() -> Weight {
		XcmGeneric::<Runtime>::refund_surplus()
	}
	fn set_error_handler(_xcm: &Xcm<()>) -> Weight {
		XcmGeneric::<Runtime>::set_error_handler()
	}
	fn set_appendix(_xcm: &Xcm<()>) -> Weight {
		XcmGeneric::<Runtime>::set_appendix()
	}
	fn clear_error() -> Weight {
		XcmGeneric::<Runtime>::clear_error()
	}
	fn claim_asset(assets: &MultiAssets, _ticket: &MultiLocation) -> Weight {
		XcmGeneric::<Runtime>::claim_asset(assets)
	}
	fn trap(code: &u64) -> Weight {
		XcmGeneric::<Runtime>::trap(code)
	}
	fn subscribe_version(_query_id: &QueryId, _max_response_weight: &u64) -> Weight {
		XcmGeneric::<Runtime>::subscribe_version()
	}
	fn unsubscribe_version() -> Weight {
		XcmGeneric::<Runtime>::unsubscribe_version()
	}
}