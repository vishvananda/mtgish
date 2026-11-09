impl Context {
    /// Last-coin predicates suffice only when each participant flipped once.
    fn one_coin_per_player(body: &Program, binding: &LoopBinding) -> bool {
        match body.instructions.as_slice() {
            [Instruction::Effect {
                operation:
                    Operation::FlipCoin {
                        player: PlayerRef::Bound(who),
                    },
                ..
            }] => who.0 == binding.0,
            [Instruction::Group(body)] => Self::one_coin_per_player(body, binding),
            _ => false,
        }
    }
    /// These references retain their player identities across intervening actions.
    fn stable_coin_players(players: &PlayerSet) -> bool {
        match players {
            PlayerSet::One(
                PlayerRef::AbilityController
                | PlayerRef::Target { .. }
                | PlayerRef::Bound(_)
                | PlayerRef::EventSubject
                | PlayerRef::EventOtherPlayer,
            ) => true,
            PlayerSet::Union(players) => players.iter().all(Self::stable_coin_players),
            _ => false,
        }
    }
    fn all_coins_heads(&self) -> Result<Condition> {
        let players = self.coin_players.as_ref().ok_or_else(|| {
            invalid("all-coin heads check has no proven one-flip-per-player batch")
        })?;
        fn heads(players: &PlayerSet) -> Condition {
            match players {
                PlayerSet::One(player) => Condition::CoinCameUp {
                    player: Some(player.clone()),
                    outcome: CoinOutcome::Heads,
                },
                PlayerSet::Union(players) => Condition::All(players.iter().map(heads).collect()),
                _ => unreachable!("only stable, explicit participants establish a coin batch"),
            }
        }
        Ok(heads(players))
    }

    /// One coin flip by the one acting player, and the result branches read.
    fn coin_flip(&mut self, actor: &PlayerSet) -> Result<(Instruction, ResultId)> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("a coin flip needs one player"));
        };
        let flip = self.effect(Operation::FlipCoin {
            player: player.clone(),
        });
        let Instruction::Effect { result, .. } = &flip else {
            unreachable!()
        };
        let result = result.clone();
        Ok((flip, result))
    }

    /// The flip forms: one branch for each way the coin can come out.
    fn coin_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Result<Option<Instruction>> {
        use source::Action as S;
        use CoinOutcome::{Heads, Lost, Won};
        let (outcome, then, otherwise): (_, &[source::Action], &[source::Action]) = match value {
            S::FlipACoin => {
                return Ok(Some(self.coin_flip(actor)?.0));
            }
            S::Reflexive_FlipACoin_WhenYouWin_WhenYouLose(won, lost) => {
                let (flip, result) = self.coin_flip(actor)?;
                let on = |outcome| {
                    Some(Condition::CoinFlip {
                        result: result.clone(),
                        outcome,
                    })
                };
                let won = self.reflexive_on(result.clone(), on(Won), won)?;
                let lost = self.reflexive_on(result.clone(), on(Lost), lost)?;
                return Ok(Some(Instruction::Group(Program {
                    instructions: vec![flip, won, lost],
                })));
            }
            S::FlipACoin_OnWin(won) => (Won, won, &[]),
            S::FlipACoin_OnLose(lost) => (Lost, lost, &[]),
            S::FlipACoin_OnWinAndLose(won, lost) => (Won, won, lost),
            S::FlipACoin_OnHeadAndOnTails(heads, tails) => (Heads, heads, tails),
            _ => return Ok(None),
        };
        let (flip, result) = self.coin_flip(actor)?;
        let branch = self.conditional(
            Condition::CoinFlip { result, outcome },
            |context| context.action_list(then, actor),
            |context| context.action_list(otherwise, actor),
        )?;
        Ok(Some(Instruction::Group(Program {
            instructions: vec![flip, branch],
        })))
    }
}
