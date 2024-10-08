use crate::errors::{InventoryError, PlayerError, RoomError};
use anchor_lang::prelude::*;

pub mod errors;

declare_id!("2HkFArK6JYkarKcynVvwc76Dt5MZFwNrjWnzWaxhzmE3");

#[program]
pub mod startup_game {

    use super::*;

    pub fn initialize_player(ctx: Context<InitializePlayer>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        if player.is_initialized {
            return err!(PlayerError::AlreadyInitialized);
        }
        player.is_initialized = true;
        player.owner = ctx.accounts.owner.key();
        player.lootbox_level = 0;
        player.silver = 0;
        player.experience = 0;
        player.clean_cash = 500;
        player.dirty_cash = 0;
        player.workers = 0;
        player.enforcers = 0;
        player.hitmen = 0;
        player.rooms = vec![];

        Ok(())
    }

    pub fn initialize_inventory(ctx: Context<InitializeInventory>) -> Result<()> {
        let inventory = &mut ctx.accounts.inventory;

        if inventory.is_initialized {
            return Err(PlayerError::AlreadyInitialized.into());
        }

        inventory.is_initialized = true;
        inventory.owner = ctx.accounts.owner.key();
        inventory.items = Vec::new();

        Ok(())
    }

    pub fn claim_lootbox(ctx: Context<ClaimLootbox>) -> Result<()> {
        let player = &mut ctx.accounts.player;

        if player.lootbox_level > 0 {
            return err!(PlayerError::LootboxAlreadyClaimed);
        }

        if player.experience < 3 {
            return err!(PlayerError::InsufficientExperience);
        }

        player.lootbox_level = 1;

        Ok(())
    }

    pub fn upgrade_lootbox(ctx: Context<UpgradeLootbox>) -> Result<()> {
        let player = &mut ctx.accounts.player;

        if player.lootbox_level == 0 {
            return err!(PlayerError::LootboxNotClaimed);
        }

        let cost = player
            .get_lootbox_upgrade_cost()
            .ok_or(PlayerError::MaxLevelReached)?;

        if player.silver < cost {
            return err!(PlayerError::InsufficientSilver);
        }

        player.silver = player
            .silver
            .checked_sub(cost)
            .ok_or(PlayerError::InsufficientSilver)?;

        player.lootbox_level += 1;

        Ok(())
    }

    pub fn purchase_room(ctx: Context<PurchaseRoom>, room_type: RoomType) -> Result<()> {
        let player = &mut ctx.accounts.player;

        // Check experience requirement
        if player.experience < room_type.experience_requirement() {
            return err!(RoomError::InsufficientExperience);
        }

        // Check if the player already owns this type of room
        if player.owns_room(room_type.clone()) {
            return err!(RoomError::RoomAlreadyOwned);
        }

        // Check if the player has enough clean cash
        let cost = room_type.cost();
        if player.clean_cash < cost {
            return err!(RoomError::InsufficientFunds);
        }

        // Deduct the cost from player's clean cash
        player.clean_cash = player
            .clean_cash
            .checked_sub(cost)
            .ok_or(RoomError::InsufficientFunds)?;

        let clock = Clock::get()?;
        let new_room = Room {
            id: player.rooms.len() as u64 + 1,
            room_type: room_type.clone(),
            level: 1,
            storage_capacity: room_type.storage_capacity(),
            last_collected: clock.unix_timestamp as u64,
        };

        player.rooms.push(new_room);

        player.experience += 1;

        match room_type {
            // Quest 1: Build Laundry
            RoomType::Laundry => player.complete_quest(0),
            // Quest 2: Build Unlicensed Bar
            RoomType::UnlicensedBar => player.complete_quest(1),
            // Quest 4: Build Fastfood Restaurant
            RoomType::FastFoodRestaurant => player.complete_quest(3),
            // Quest 5: Build Security Room
            RoomType::SecurityRoom => player.complete_quest(4),
            // Quest 7: Build Canabis Farm
            RoomType::CannabisFarm => player.complete_quest(6),
            // Quest 8: Build Saferoom
            RoomType::Saferoom => player.complete_quest(7),
            // Quest 9: Build Strip Club
            RoomType::StripClub => player.complete_quest(8),
            // Quest 10: Build Casino
            RoomType::Casino => player.complete_quest(9),
            // Quest 11: Build Fitness Center
            RoomType::FitnessCenter => player.complete_quest(10),
            _ => {}
        }

        Ok(())
    }

    pub fn collect_dirty_cash(ctx: Context<CollectDirtyCash>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;

        let mut total_dirty_cash = 0;

        for room in player.rooms.iter_mut() {
            if matches!(
                room.room_type,
                RoomType::UnlicensedBar
                    | RoomType::CannabisFarm
                    | RoomType::StripClub
                    | RoomType::Casino
            ) {
                let pending_reward = room.pending_rewards();
                if pending_reward > 0 {
                    total_dirty_cash += pending_reward;
                    room.last_collected = clock.unix_timestamp as u64;
                }
            }
        }

        if total_dirty_cash > 0 {
            player.dirty_cash += total_dirty_cash;
        }

        Ok(())
    }

    pub fn collect_clean_cash(ctx: Context<CollectCleanCash>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;

        let available_dirty_cash = player.dirty_cash;
        if available_dirty_cash == 0 {
            return Err(RoomError::InsufficientFunds.into());
        }

        let mut total_launderable_amount = 0;

        // Calculate the total amount that can be laundered
        for room in player.rooms.iter_mut() {
            if matches!(
                room.room_type,
                RoomType::Laundry | RoomType::FastFoodRestaurant | RoomType::FitnessCenter
            ) {
                let pending_reward = room.pending_rewards();
                if pending_reward > 0 {
                    total_launderable_amount += pending_reward;
                    room.last_collected = clock.unix_timestamp as u64;
                }
            }
        }

        let convertible_dirty_cash = total_launderable_amount.min(available_dirty_cash);

        msg!("Available dirty cash: {}", available_dirty_cash);
        msg!("Total launderable amount: {}", total_launderable_amount);
        msg!("Convertible dirty cash: {}", convertible_dirty_cash);

        if convertible_dirty_cash > 0 {
            // 30% loss on dirty -> clean cash conversion
            let clean_cash_produced = (convertible_dirty_cash as f64 * 0.7).round() as u64;
            msg!("Clean cash produced: {}", clean_cash_produced);
            player.clean_cash += clean_cash_produced;
            player.dirty_cash = player
                .dirty_cash
                .checked_sub(convertible_dirty_cash)
                .ok_or(RoomError::InsufficientFunds)?;

            // Check if clean cash >= $600 for Quest 3
            if !player.is_quest_completed(2) && player.clean_cash >= 600 {
                player.complete_quest(2);
            }
        }

        Ok(())
    }

    pub fn recruit_units(ctx: Context<RecruitUnits>, enforcers: u64, hitmen: u64) -> Result<()> {
        let player = &mut ctx.accounts.player;

        let security_room = player
            .rooms
            .iter()
            .find(|room| room.room_type == RoomType::SecurityRoom);
        if security_room.is_none() {
            return err!(RoomError::NoSecurityRoom);
        }

        // Hitmen can only be recruited if player has a Security Room at level 2 or higher
        /*
        if hitmen > 0 {
            if let Some(room) = security_room {
                if room.level < 2 {
                    return err!(RoomError::SecurityRoomLevelTooLow);
                }
            }
        }
        */

        // One enforcer costs $50 clean cash
        let enforcer_cost = enforcers.checked_mul(50).ok_or(RoomError::Overflow)?;
        if player.clean_cash < enforcer_cost {
            return err!(RoomError::InsufficientFunds);
        }

        // One hitman costs $100 dirty cash
        let hitmen_cost = hitmen.checked_mul(100).ok_or(RoomError::Overflow)?;
        if player.dirty_cash < hitmen_cost {
            return err!(RoomError::InsufficientFunds);
        }

        if enforcers > 0 {
            player.clean_cash = player
                .clean_cash
                .checked_sub(enforcer_cost)
                .ok_or(RoomError::InsufficientFunds)?;
            player.enforcers = player
                .enforcers
                .checked_add(enforcers)
                .ok_or(RoomError::Overflow)?;
        }

        if hitmen > 0 {
            player.dirty_cash = player
                .dirty_cash
                .checked_sub(hitmen_cost)
                .ok_or(RoomError::InsufficientFunds)?;
            player.hitmen = player
                .hitmen
                .checked_add(hitmen)
                .ok_or(RoomError::Overflow)?;
        }

        // Check if the player has recruited at least 10 enforcers and 10 hitmen for Quest 6
        if !player.is_quest_completed(5) && player.enforcers >= 10 && player.hitmen >= 10 {
            player.complete_quest(5);
        }

        Ok(())
    }

    pub fn recruit_team_member(
        ctx: Context<RecruitTeamMember>,
        member: InventoryItem,
    ) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let inventory = &mut ctx.accounts.inventory;

        if !inventory.is_initialized {
            return err!(InventoryError::UninitializedAccount);
        }

        // Check if the team member is allowed (for now, only Thief)
        if !member.is_allowed() {
            return err!(InventoryError::InvalidItem);
        }

        if inventory.has_team_member(member.clone()) {
            return err!(InventoryError::AlreadyRecruited);
        }

        if player.experience < 9 {
            return err!(InventoryError::InsufficientExperience);
        }

        if player.dirty_cash < 5000 {
            return err!(InventoryError::InsufficientFunds);
        }

        player.dirty_cash = player
            .dirty_cash
            .checked_sub(5000)
            .ok_or(InventoryError::InsufficientFunds)?;

        inventory.add_team_member(member);

        player.complete_quest(Player::QUEST_RECRUIT_THIEF);
        player.experience += 1;

        Ok(())
    }

    pub fn claim_quest_reward(ctx: Context<ClaimQuestReward>, quest_id: u8) -> Result<()> {
        let player = &mut ctx.accounts.player;

        if !player.is_quest_completed(quest_id) {
            return err!(PlayerError::QuestNotCompleted);
        }

        if player.is_quest_claimed(quest_id) {
            return err!(PlayerError::RewardAlreadyClaimed);
        }

        player.silver += 100;

        player.claim_quest_reward(quest_id);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePlayer<'info> {
    #[account(
        init,
        payer = owner,
        space = 5000,
        seeds = [b"PLAYER", owner.key().as_ref()],
        bump
    )]
    pub player: Account<'info, Player>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeInventory<'info> {
    #[account(
        init,
        payer = owner,
        space = 5000,
        seeds = [b"INVENTORY", owner.key().as_ref()],
        bump
    )]
    pub inventory: Account<'info, Inventory>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Inventory {
    pub is_initialized: bool,
    pub owner: Pubkey,
    pub items: Vec<InventoryItem>,
}

impl Inventory {
    fn has_team_member(&self, member: InventoryItem) -> bool {
        self.items.contains(&member)
    }

    fn add_team_member(&mut self, member: InventoryItem) {
        self.items.push(member);
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
pub enum InventoryItem {
    Thief,
    Diplomat,
    Researcher,
    PromoLootBox,
}

impl InventoryItem {
    fn is_allowed(&self) -> bool {
        matches!(self, InventoryItem::Thief)
    }
}

#[derive(Accounts)]
pub struct ClaimLootbox<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpgradeLootbox<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct PurchaseRoom<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct CollectDirtyCash<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct CollectCleanCash<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct RecruitUnits<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct RecruitTeamMember<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    #[account(mut, has_one = owner)]
    pub inventory: Account<'info, Inventory>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimQuestReward<'info> {
    #[account(mut, has_one = owner)]
    pub player: Account<'info, Player>,
    pub owner: Signer<'info>,
}

#[account]
pub struct Player {
    pub is_initialized: bool,
    pub owner: Pubkey,
    pub lootbox_level: u8,
    pub silver: u64,
    pub experience: u64,
    pub clean_cash: u64,
    pub dirty_cash: u64,
    pub workers: u64,
    pub enforcers: u64,
    pub hitmen: u64,
    pub quest_completion_bitmask: u64,
    pub quest_claim_bitmask: u64,
    pub rooms: Vec<Room>,
}

impl Player {
    const QUEST_RECRUIT_THIEF: u8 = 11;

    fn owns_room(&self, room_type: RoomType) -> bool {
        self.rooms.iter().any(|room| room.room_type == room_type)
    }

    fn complete_quest(&mut self, quest_id: u8) {
        if quest_id < 64 {
            self.quest_completion_bitmask |= 1 << quest_id;
        }
    }

    fn is_quest_completed(&self, quest_id: u8) -> bool {
        if quest_id < 64 {
            (self.quest_completion_bitmask & (1 << quest_id)) != 0
        } else {
            false
        }
    }

    fn claim_quest_reward(&mut self, quest_id: u8) {
        if quest_id < 64 {
            self.quest_claim_bitmask |= 1 << quest_id;
        }
    }

    fn is_quest_claimed(&self, quest_id: u8) -> bool {
        if quest_id < 64 {
            (self.quest_claim_bitmask & (1 << quest_id)) != 0
        } else {
            false
        }
    }

    fn get_lootbox_upgrade_cost(&self) -> Option<u64> {
        match self.lootbox_level {
            1 => Some(1000),
            2 => Some(2400),
            3 => Some(3800),
            _ => None,
        }
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Room {
    pub id: u64,
    pub room_type: RoomType,
    pub level: u64,
    pub storage_capacity: u64,
    pub last_collected: u64,
}

impl Room {
    fn pending_rewards(&self) -> u64 {
        let clock = Clock::get().unwrap();
        let elapsed_time = (clock.unix_timestamp as u64).saturating_sub(self.last_collected);
        let yield_per_second = self.room_type.yield_per_minute() as f64 / 60.0;
        let potential_reward = (elapsed_time as f64 * yield_per_second).round() as u64;
        potential_reward.min(self.storage_capacity)
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
pub enum RoomType {
    // Legal businesses
    Laundry,
    FastFoodRestaurant,
    FitnessCenter,
    // Illegal businesses
    UnlicensedBar,
    CannabisFarm,
    StripClub,
    Casino,
    // Supporting rooms
    Saferoom,
    SecurityRoom,
}

impl RoomType {
    fn cost(&self) -> u64 {
        match self {
            RoomType::Laundry => 100,
            RoomType::FastFoodRestaurant => 600,
            RoomType::FitnessCenter => 800,
            RoomType::UnlicensedBar => 400,
            RoomType::CannabisFarm => 500,
            RoomType::StripClub => 1500,
            RoomType::Casino => 2000,
            RoomType::Saferoom => 800,
            RoomType::SecurityRoom => 600,
        }
    }

    fn experience_requirement(&self) -> u64 {
        match self {
            RoomType::Laundry => 0,
            RoomType::FastFoodRestaurant => 2,
            RoomType::FitnessCenter => 5,
            RoomType::UnlicensedBar => 1,
            RoomType::CannabisFarm => 3,
            RoomType::StripClub => 4,
            RoomType::Casino => 6,
            RoomType::Saferoom => 0,
            RoomType::SecurityRoom => 2,
        }
    }

    fn storage_capacity(&self) -> u64 {
        match self {
            RoomType::Laundry => 100,
            RoomType::FastFoodRestaurant => 200,
            RoomType::FitnessCenter => 300,
            RoomType::UnlicensedBar => 150,
            RoomType::CannabisFarm => 250,
            RoomType::StripClub => 400,
            RoomType::Casino => 500,
            RoomType::Saferoom => 300,
            RoomType::SecurityRoom => 0,
        }
    }

    fn yield_per_minute(&self) -> u64 {
        match self {
            RoomType::Laundry => 50,
            RoomType::FastFoodRestaurant => 75,
            RoomType::FitnessCenter => 85,
            RoomType::UnlicensedBar => 65,
            RoomType::CannabisFarm => 70,
            RoomType::StripClub => 100,
            RoomType::Casino => 120,
            RoomType::Saferoom => 0,
            RoomType::SecurityRoom => 0,
        }
    }
}

impl RoomType {
    fn upgraded_cost(&self, level: u8) -> u64 {
        (self.cost() as f64 * 1.10_f64.powi(level as i32 - 1)) as u64
    }

    fn upgraded_yield_per_minute(&self, level: u8) -> u64 {
        (self.yield_per_minute() as f64 * 1.10_f64.powi(level as i32 - 1)) as u64
    }

    fn upgraded_storage_capacity(&self, level: u8) -> u64 {
        (self.storage_capacity() as f64 * 1.10_f64.powi(level as i32 - 1)) as u64
    }
}
