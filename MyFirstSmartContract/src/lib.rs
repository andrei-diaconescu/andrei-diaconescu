#![no_std]
#![allow(unused_attributes)]
#![allow(non_snake_case)]

elrond_wasm::imports!();
elrond_wasm::derive_imports!();

#[derive(TopEncode, TopDecode, PartialEq, TypeAbi)]
pub struct Payment<BigUint: BigUintApi> {
    pub donor: Address,
    pub claimer: Address,
    pub first_payday: u64,
    pub second_payday: u64,
    pub reclaim_payday: u64,
    pub funds: BigUint,
}

#[elrond_wasm_derive::contract(FirstScImpl)]
pub trait FirstSc {
    #[init]
    fn init(&self) {
        self.set_deal_id(0);
    }

    //endpoints

    #[payable("EGLD")]
    #[endpoint]
    fn fund(
        &self,
        #[payment] payment: BigUint,
        to: Address,
        first_deadline: u64,
        second_deadline: u64,
        reclaim_deadline: u64,
    ) -> SCResult<()> {
        require!(
            first_deadline < second_deadline
                && reclaim_deadline > first_deadline
                && reclaim_deadline > second_deadline
                && first_deadline > self.get_block_epoch(),
            "deadlines must be in a correct order"
        );

        let caller = self.get_caller();
        let deal = Payment::<BigUint> {
            donor: caller,
            claimer: to,
            first_payday: first_deadline,
            second_payday: second_deadline,
            reclaim_payday: reclaim_deadline,
            funds: payment,
        };

        let mut id = self.get_deal_id();
        self.get_deal_mapper().insert(id, deal);
        id += 1;
        self.set_deal_id(id);
        Ok(())
    }

    #[endpoint]
    fn claim(&self, deal_id: u64) -> SCResult<()> {
        require!(
            self.get_deal_mapper().contains_key(&deal_id),
            "this deal does not exist or was already finished"
        );
        let deal = self.get_deal_mapper().get(&deal_id).unwrap();
        let mut funds = deal.funds;
        let claimer = deal.claimer.clone();

        if self.get_caller() == claimer {
            let mut first_deadline = deal.first_payday;
            let second_deadline = deal.second_payday;
            if first_deadline != 0 && self.get_block_epoch() > first_deadline {
                let payment = funds.clone() / BigUint::from(2u64);
                self.send().direct_egld(
                    &self.get_caller(),
                    &payment,
                    b"first claim went successfully",
                );
                funds /= BigUint::from(2u64);
                first_deadline = 0;
                let updated_deal = Payment::<BigUint> {
                    donor: deal.donor,
                    claimer: deal.claimer,
                    first_payday: first_deadline,
                    second_payday: second_deadline,
                    reclaim_payday: deal.reclaim_payday,
                    funds,
                };
                self.get_deal_mapper().insert(deal_id, updated_deal);
                return Ok(());
            } else if self.get_block_epoch() > second_deadline {
                self.send().direct_egld(
                    &self.get_caller(),
                    &funds,
                    b"second claim went successfully",
                );
                self.get_deal_mapper().remove(&deal_id);
                return Ok(());
            }
        } else if self.get_caller() == deal.donor {
            let third_deadline = deal.reclaim_payday;
            if self.get_block_epoch() > third_deadline {
                let donor = deal.donor;
                self.send()
                    .direct_egld(&donor, &funds, b"reclaim went successfully");
                self.get_deal_mapper().remove(&deal_id);
                return Ok(());
            } else {
                return sc_error!("You can only reclaim after the 3rd deadline.");
            }
        } else {
            return sc_error!("You are not part of this deal.");
        }
        sc_error!("You need to wait")
    }

    #[storage_mapper("deals")]
    fn get_deal_mapper(&self) -> MapMapper<Self::Storage, u64, Payment<BigUint>>;

    #[view]
    #[storage_get("dealId")]
    fn get_deal_id(&self) -> u64;

    #[storage_set("dealId")]
    fn set_deal_id(&self, value: u64);
}
