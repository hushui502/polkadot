// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! This module is responsible for maintaining a consistent initialization order for all other
//! parachains modules. It's also responsible for finalization and session change notifications.
//!
//! This module can throw fatal errors if session-change notifications are received after initialization.

use crate::{
	configuration::{self, HostConfiguration},
	disputes::DisputesHandler,
	dmp, hrmp, inclusion, paras, scheduler, session_info, shared, ump,
};
use frame_support::{
	traits::{OneSessionHandler, Randomness},
	weights::Weight,
};
use frame_system::limits::BlockWeights;
use parity_scale_codec::{Decode, Encode};
use primitives::v1::{BlockNumber, ConsensusLog, SessionIndex, ValidatorId};
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;

/// Information about a session change that has just occurred.
#[derive(Clone)]
pub struct SessionChangeNotification<BlockNumber> {
	/// The new validators in the session.
	pub validators: Vec<ValidatorId>,
	/// The queued validators for the following session.
	pub queued: Vec<ValidatorId>,
	/// The configuration before handling the session change
	pub prev_config: HostConfiguration<BlockNumber>,
	/// The configuration after handling the session change.
	pub new_config: HostConfiguration<BlockNumber>,
	/// A secure random seed for the session, gathered from BABE.
	pub random_seed: [u8; 32],
	/// New session index.
	pub session_index: SessionIndex,
}

impl<BlockNumber: Default + From<u32>> Default for SessionChangeNotification<BlockNumber> {
	fn default() -> Self {
		Self {
			validators: Vec::new(),
			queued: Vec::new(),
			prev_config: HostConfiguration::default(),
			new_config: HostConfiguration::default(),
			random_seed: Default::default(),
			session_index: Default::default(),
		}
	}
}

#[derive(Encode, Decode, TypeInfo)]
struct BufferedSessionChange {
	validators: Vec<ValidatorId>,
	queued: Vec<ValidatorId>,
	session_index: SessionIndex,
}

pub trait WeightInfo {
	fn force_approve(d: u32) -> Weight;
}

impl WeightInfo for () {
	fn force_approve(_: u32) -> Weight {
		BlockWeights::default().max_block
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ configuration::Config
		+ shared::Config
		+ paras::Config
		+ scheduler::Config
		+ inclusion::Config
		+ session_info::Config
		+ dmp::Config
		+ ump::Config
		+ hrmp::Config
	{
		/// A randomness beacon.
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		/// An origin which is allowed to force updates to parachains.
		type ForceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Whether the parachains modules have been initialized within this block.
	///
	/// Semantically a `bool`, but this guarantees it should never hit the trie,
	/// as this is cleared in `on_finalize` and Frame optimizes `None` values to be empty values.
	///
	/// As a `bool`, `set(false)` and `remove()` both lead to the next `get()` being false, but one of
	/// them writes to the trie and one does not. This confusion makes `Option<()>` more suitable for
	/// the semantics of this variable.
	#[pallet::storage]
	pub(super) type HasInitialized<T: Config> = StorageValue<_, ()>;

	/// Buffered session changes along with the block number at which they should be applied.
	///
	/// Typically this will be empty or one element long. Apart from that this item never hits
	/// the storage.
	///
	/// However this is a `Vec` regardless to handle various edge cases that may occur at runtime
	/// upgrade boundaries or if governance intervenes.
	#[pallet::storage]
	pub(super) type BufferedSessionChanges<T: Config> =
		StorageValue<_, Vec<BufferedSessionChange>, ValueQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// The other modules are initialized in this order:
			// - Configuration
			// - Paras
			// - Scheduler
			// - Inclusion
			// - `SessionInfo`
			// - Disputes
			// - DMP
			// - UMP
			// - HRMP
			let total_weight = configuration::Pallet::<T>::initializer_initialize(now) +
				shared::Pallet::<T>::initializer_initialize(now) +
				paras::Pallet::<T>::initializer_initialize(now) +
				scheduler::Pallet::<T>::initializer_initialize(now) +
				inclusion::Pallet::<T>::initializer_initialize(now) +
				session_info::Pallet::<T>::initializer_initialize(now) +
				T::DisputesHandler::initializer_initialize(now) +
				dmp::Pallet::<T>::initializer_initialize(now) +
				ump::Pallet::<T>::initializer_initialize(now) +
				hrmp::Pallet::<T>::initializer_initialize(now);

			HasInitialized::<T>::set(Some(()));

			total_weight
		}

		fn on_finalize(_: T::BlockNumber) {
			// reverse initialization order.
			hrmp::Pallet::<T>::initializer_finalize();
			ump::Pallet::<T>::initializer_finalize();
			dmp::Pallet::<T>::initializer_finalize();
			T::DisputesHandler::initializer_finalize();
			session_info::Pallet::<T>::initializer_finalize();
			inclusion::Pallet::<T>::initializer_finalize();
			scheduler::Pallet::<T>::initializer_finalize();
			paras::Pallet::<T>::initializer_finalize();
			shared::Pallet::<T>::initializer_finalize();
			configuration::Pallet::<T>::initializer_finalize();

			// Apply buffered session changes as the last thing. This way the runtime APIs and the
			// next block will observe the next session.
			//
			// Note that we only apply the last session as all others lasted less than a block (weirdly).
			if let Some(BufferedSessionChange { session_index, validators, queued }) =
				BufferedSessionChanges::<T>::take().pop()
			{
				Self::apply_new_session(session_index, validators, queued);
			}

			HasInitialized::<T>::take();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issue a signal to the consensus engine to forcibly act as though all parachain
		/// blocks in all relay chain blocks up to and including the given number in the current
		/// chain are valid and should be finalized.
		#[pallet::weight((
			<T as Config>::WeightInfo::force_approve(
				frame_system::Pallet::<T>::digest().logs.len() as u32,
			),
			DispatchClass::Operational,
		))]
		pub fn force_approve(origin: OriginFor<T>, up_to: BlockNumber) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			frame_system::Pallet::<T>::deposit_log(ConsensusLog::ForceApprove(up_to).into());
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn apply_new_session(
		session_index: SessionIndex,
		all_validators: Vec<ValidatorId>,
		queued: Vec<ValidatorId>,
	) {
		let random_seed = {
			let mut buf = [0u8; 32];
			// TODO: audit usage of randomness API
			// https://github.com/paritytech/polkadot/issues/2601
			let (random_hash, _) = T::Randomness::random(&b"paras"[..]);
			let len = sp_std::cmp::min(32, random_hash.as_ref().len());
			buf[..len].copy_from_slice(&random_hash.as_ref()[..len]);
			buf
		};

		let configuration::SessionChangeOutcome { prev_config, new_config } =
			configuration::Pallet::<T>::initializer_on_new_session(&session_index);
		let new_config = new_config.unwrap_or_else(|| prev_config.clone());

		let validators = shared::Pallet::<T>::initializer_on_new_session(
			session_index,
			random_seed.clone(),
			&new_config,
			all_validators,
		);

		let notification = SessionChangeNotification {
			validators,
			queued,
			prev_config,
			new_config,
			random_seed,
			session_index,
		};

		let outgoing_paras = paras::Pallet::<T>::initializer_on_new_session(&notification);
		scheduler::Pallet::<T>::initializer_on_new_session(&notification);
		inclusion::Pallet::<T>::initializer_on_new_session(&notification);
		session_info::Pallet::<T>::initializer_on_new_session(&notification);
		T::DisputesHandler::initializer_on_new_session(&notification);
		dmp::Pallet::<T>::initializer_on_new_session(&notification, &outgoing_paras);
		ump::Pallet::<T>::initializer_on_new_session(&notification, &outgoing_paras);
		hrmp::Pallet::<T>::initializer_on_new_session(&notification, &outgoing_paras);
	}

	/// Should be called when a new session occurs. Buffers the session notification to be applied
	/// at the end of the block. If `queued` is `None`, the `validators` are considered queued.
	fn on_new_session<'a, I: 'a>(
		_changed: bool,
		session_index: SessionIndex,
		validators: I,
		queued: Option<I>,
	) where
		I: Iterator<Item = (&'a T::AccountId, ValidatorId)>,
	{
		let validators: Vec<_> = validators.map(|(_, v)| v).collect();
		let queued: Vec<_> = if let Some(queued) = queued {
			queued.map(|(_, v)| v).collect()
		} else {
			validators.clone()
		};

		if session_index == 0 {
			// Genesis session should be immediately enacted.
			Self::apply_new_session(0, validators, queued);
		} else {
			BufferedSessionChanges::<T>::mutate(|v| {
				v.push(BufferedSessionChange { validators, queued, session_index })
			});
		}
	}

	// Allow to trigger `on_new_session` in tests, this is needed as long as `pallet_session` is not
	// implemented in mock.
	#[cfg(any(test, feature = "runtime-benchmarks"))]
	pub(crate) fn test_trigger_on_new_session<'a, I: 'a>(
		changed: bool,
		session_index: SessionIndex,
		validators: I,
		queued: Option<I>,
	) where
		I: Iterator<Item = (&'a T::AccountId, ValidatorId)>,
	{
		Self::on_new_session(changed, session_index, validators, queued)
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = ValidatorId;
}

impl<T: pallet_session::Config + Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = ValidatorId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
	{
		<Pallet<T>>::on_new_session(false, 0, validators, None);
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued: I)
	where
		I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
	{
		let session_index = <pallet_session::Pallet<T>>::current_index();
		<Pallet<T>>::on_new_session(changed, session_index, validators, Some(queued));
	}

	fn on_disabled(_i: u32) {}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{
		new_test_ext, Configuration, Dmp, Initializer, MockGenesisConfig, Paras, SessionInfo,
		System,
	};
	use primitives::v1::{HeadData, Id as ParaId};
	use test_helpers::dummy_validation_code;

	use frame_support::{
		assert_ok,
		traits::{OnFinalize, OnInitialize},
	};

	#[test]
	fn session_0_is_instantly_applied() {
		new_test_ext(Default::default()).execute_with(|| {
			Initializer::on_new_session(
				false,
				0,
				Vec::new().into_iter(),
				Some(Vec::new().into_iter()),
			);

			let v = <Initializer as Store>::BufferedSessionChanges::get();
			assert!(v.is_empty());

			assert_eq!(SessionInfo::earliest_stored_session(), 0);
			assert!(SessionInfo::session_info(0).is_some());
		});
	}

	#[test]
	fn session_change_before_initialize_is_still_buffered_after() {
		new_test_ext(Default::default()).execute_with(|| {
			Initializer::on_new_session(
				false,
				1,
				Vec::new().into_iter(),
				Some(Vec::new().into_iter()),
			);

			let now = System::block_number();
			Initializer::on_initialize(now);

			let v = <Initializer as Store>::BufferedSessionChanges::get();
			assert_eq!(v.len(), 1);
		});
	}

	#[test]
	fn session_change_applied_on_finalize() {
		new_test_ext(Default::default()).execute_with(|| {
			Initializer::on_initialize(1);
			Initializer::on_new_session(
				false,
				1,
				Vec::new().into_iter(),
				Some(Vec::new().into_iter()),
			);

			Initializer::on_finalize(1);

			assert!(<Initializer as Store>::BufferedSessionChanges::get().is_empty());
		});
	}

	#[test]
	fn sets_flag_on_initialize() {
		new_test_ext(Default::default()).execute_with(|| {
			Initializer::on_initialize(1);

			assert!(<Initializer as Store>::HasInitialized::get().is_some());
		})
	}

	#[test]
	fn clears_flag_on_finalize() {
		new_test_ext(Default::default()).execute_with(|| {
			Initializer::on_initialize(1);
			Initializer::on_finalize(1);

			assert!(<Initializer as Store>::HasInitialized::get().is_none());
		})
	}

	#[test]
	fn scheduled_cleanup_performed() {
		let a = ParaId::from(1312);
		let b = ParaId::from(228);
		let c = ParaId::from(123);

		let mock_genesis = crate::paras::ParaGenesisArgs {
			parachain: true,
			genesis_head: HeadData(vec![4, 5, 6]),
			validation_code: dummy_validation_code(),
		};

		new_test_ext(MockGenesisConfig {
			configuration: crate::configuration::GenesisConfig {
				config: crate::configuration::HostConfiguration {
					max_downward_message_size: 1024,
					..Default::default()
				},
			},
			paras: crate::paras::GenesisConfig {
				paras: vec![
					(a, mock_genesis.clone()),
					(b, mock_genesis.clone()),
					(c, mock_genesis.clone()),
				],
				..Default::default()
			},
			..Default::default()
		})
		.execute_with(|| {
			// enqueue downward messages to A, B and C.
			assert_ok!(Dmp::queue_downward_message(&Configuration::config(), a, vec![1, 2, 3]));
			assert_ok!(Dmp::queue_downward_message(&Configuration::config(), b, vec![4, 5, 6]));
			assert_ok!(Dmp::queue_downward_message(&Configuration::config(), c, vec![7, 8, 9]));

			assert_ok!(Paras::schedule_para_cleanup(a));
			assert_ok!(Paras::schedule_para_cleanup(b));

			// Apply session 2 in the future
			Initializer::apply_new_session(2, vec![], vec![]);

			assert!(Dmp::dmq_contents(a).is_empty());
			assert!(Dmp::dmq_contents(b).is_empty());
			assert!(!Dmp::dmq_contents(c).is_empty());
		});
	}
}
