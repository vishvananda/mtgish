impl Context {
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
