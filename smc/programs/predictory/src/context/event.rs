use anchor_lang::prelude::*;

use crate::{
    context::{withdraw_sol, COMPLETION_DEADLINE, UUID_VERSION},
    error::ProgramError,
    id,
    state::{
        contract_state::State,
        event::{Event, EventMeta},
        user::User,
    },
};
// --------------------------- Context ----------------------------- //

#[derive(Accounts)]
#[instruction(
    event_id: u128,
)]
pub struct CreateEvent<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"user".as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub user: Account<'info, User>,

    #[account(
        seeds = [b"state".as_ref()],
        bump,
    )]
    pub state: Account<'info, State>,

    #[account(
        init,
        payer = authority,
        owner = id(),
        seeds = [b"event".as_ref(), &event_id.to_le_bytes()],
        bump,
        space = Event::LEN
    )]
    pub event: Account<'info, Event>,

    #[account(
        init,
        payer = authority,
        owner = id(),
        seeds = [b"event_meta".as_ref(), &event_id.to_le_bytes()],
        bump,
        space = EventMeta::LEN
    )]
    pub event_meta: Account<'info, EventMeta>,

    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    event_id: u128,
)]
pub struct UpdateEvent<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"event".as_ref(), &event_id.to_le_bytes()],
        constraint = event.authority == authority.key() @ ProgramError::AuthorityMismatch,
        constraint = event.start_date > Clock::get()?.unix_timestamp @ ProgramError::EventAlreadyStarted,
        bump,
    )]
    pub event: Account<'info, Event>,

    #[account(
        mut,
        seeds = [b"event_meta".as_ref(), &event_id.to_le_bytes()],
        bump,
    )]
    pub event_meta: Account<'info, EventMeta>,
}

#[derive(Accounts)]
#[instruction(
    event_id: u128,
)]
pub struct CancelEvent<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    /// CHECK: this is admin account
    #[account(mut)]
    pub contract_admin: UncheckedAccount<'info>,

    #[account(
        seeds = [b"state".as_ref()],
        constraint = state.authority == contract_admin.key() @ ProgramError::AuthorityMismatch,
        bump,
    )]
    pub state: Account<'info, State>,

    #[account(
        mut,
        seeds = [b"user".as_ref(), event.authority.as_ref()],
        bump,
    )]
    pub user: Account<'info, User>,

    #[account(
        mut,
        seeds = [b"event".as_ref(), &event_id.to_le_bytes()],
        bump,
    )]
    pub event: Account<'info, Event>,
}

#[derive(Accounts)]
#[instruction(
    event_id: u128,
)]
pub struct CompleteEvent<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"state".as_ref()],
        bump,
    )]
    pub state: Account<'info, State>,

    #[account(
        mut,
        seeds = [b"event".as_ref(), &event_id.to_le_bytes()],
        constraint = event.authority == authority.key() @ ProgramError::AuthorityMismatch,
        constraint = event.end_date < Clock::get()?.unix_timestamp @ ProgramError::EventIsNotOver,
        bump,
    )]
    pub event: Account<'info, Event>,
}

// -------------------------- Arguments ---------------------------- //

#[derive(AnchorDeserialize, AnchorSerialize)]
pub struct CreateEventArgs {
    name: [u8; 32],
    is_private: bool,
    description: [u8; 256],
    start_date: i64,
    end_date: i64,
    participation_deadline: Option<i64>,
}

// ------------------------ Implementation ------------------------- //

impl CreateEvent<'_> {
    pub fn create_event(&mut self, event_id: u128, args: CreateEventArgs) -> Result<()> {
        let id = uuid::Uuid::from_u128(event_id);
        self.validate(id, &args)?;

        let user = &mut self.user;

        require!(
            user.stake >= self.state.event_price,
            ProgramError::StakeTooLow
        );

        let event = &mut self.event;
        let event_meta = &mut self.event_meta;
        let stake = self.state.event_price;

        event.id = event_id;
        event.authority = self.authority.key();
        event.stake = stake;
        event.start_date = args.start_date;
        event.end_date = args.end_date;
        event.participation_deadline = args.participation_deadline;
        event.version = Event::VERSION;

        event_meta.is_private = args.is_private;
        event_meta.event_id = event_id;
        event_meta.description = args.description;
        event_meta.name = args.name;
        event_meta.version = EventMeta::VERSION;

        user.stake -= stake;
        user.locked_stake += stake;

        withdraw_sol(
            &self.user.to_account_info(),
            &self.event.to_account_info(),
            stake,
        )?;

        msg!("Event {} initialized", id);

        Ok(())
    }

    pub fn validate(&mut self, id: uuid::Uuid, args: &CreateEventArgs) -> Result<()> {
        require!(
            id.get_version_num() == UUID_VERSION,
            ProgramError::InvalidUUID
        );

        require!(
            args.start_date < args.end_date,
            ProgramError::InvalidEndDate
        );

        if let Some(deadline) = args.participation_deadline {
            require!(
                (args.start_date..=args.end_date).contains(&deadline),
                ProgramError::InvalidEndDate
            );
        }

        Ok(())
    }
}

impl UpdateEvent<'_> {
    pub fn update_event_name(&mut self, _event_id: u128, name: [u8; 32]) -> Result<()> {
        let event_meta = &mut self.event_meta;

        event_meta.name = name;

        msg!("Event name updated");

        Ok(())
    }

    pub fn update_event_description(
        &mut self,
        _event_id: u128,
        description: [u8; 256],
    ) -> Result<()> {
        let event_meta = &mut self.event_meta;

        event_meta.description = description;

        msg!("Event description updated");

        Ok(())
    }

    pub fn update_event_end_date(&mut self, _event_id: u128, end_date: i64) -> Result<()> {
        let event = &mut self.event;

        require!(event.start_date < end_date, ProgramError::InvalidEndDate);
        if let Some(deadline) = event.participation_deadline {
            require!(deadline <= end_date, ProgramError::InvalidEndDate);
        }

        event.end_date = end_date;

        msg!("Event end date updated");

        Ok(())
    }

    pub fn update_event_participation_deadline(
        &mut self,
        _event_id: u128,
        participation_deadline: Option<i64>,
    ) -> Result<()> {
        let event = &mut self.event;

        if let Some(deadline) = participation_deadline {
            require!(
                (event.start_date..=event.end_date).contains(&deadline),
                ProgramError::InvalidEndDate
            );
        }

        event.participation_deadline = participation_deadline;

        msg!("Event end participation deadline updated");

        Ok(())
    }
}

impl CancelEvent<'_> {
    pub fn cancel_event(&mut self, event_id: u128) -> Result<()> {
        let event_acc = self.event.to_account_info();
        let event = &mut self.event;

        let now = Clock::get()?.unix_timestamp;

        require!(
            event.authority == self.sender.key() || now > event.end_date + COMPLETION_DEADLINE,
            ProgramError::AuthorityMismatch
        );

        // TODO: what happens with his trust coins?
        // TODO: Do i need to add appell on appel?
        if event.start_date <= now {
            msg!("Event is already started, returning stake to contract admin");

            self.user.locked_stake -= event.stake;

            withdraw_sol(
                &event_acc,
                &self.contract_admin.to_account_info(),
                event.stake,
            )?;
        } else {
            msg!("Event is not started yet, returning stake to user");

            self.user.locked_stake -= event.stake;
            self.user.stake += event.stake;

            withdraw_sol(&event_acc, &self.user.to_account_info(), event.stake)?;
        }

        event.canceled = true;

        msg!("Event cancelled: {}", uuid::Uuid::from_u128(event_id));

        Ok(())
    }
}

impl CompleteEvent<'_> {
    pub fn complete_event(&mut self, event_id: u128, result: u8) -> Result<()> {
        let event = &mut self.event;

        event.result = Some(result);

        msg!(
            "Event completed, result - {}: {}",
            result,
            uuid::Uuid::from_u128(event_id)
        );

        Ok(())
    }
}
