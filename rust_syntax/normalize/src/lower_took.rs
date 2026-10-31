// Who did what in the last "each player" batch: "each player who did", "those who didn't".
impl Context {
    /// What this player's share of the last `done` moved.
    fn moved_by(
        &self,
        done: Done,
        phase: ObjectPhase,
        player: &PlayerRef,
        filter: Option<ObjectFilter>,
    ) -> Result<ObjectSet> {
        let moved = ObjectSet::MovedBy {
            result: self.done(done)?,
            phase,
            player: player.clone(),
        };
        Ok(match filter {
            Some(filter) => ObjectSet::Matching {
                objects: Box::new(moved),
                filter,
            },
            None => moved,
        })
    }

    /// `Ok(None)` when this filter is not about a batch's per-player result.
    fn took_check(&self, player: &PlayerRef, value: &source::Players) -> Result<Option<Condition>> {
        use source::Players as S;
        use ObjectPhase::{After, Before};
        let some = |objects: ObjectSet| Condition::Compare {
            left: Expr::CountObjects(Box::new(objects)),
            comparison: Comparison::Greater,
            right: Expr::Constant(0),
        };
        let none = |objects: ObjectSet| Condition::Not(Box::new(some(objects)));
        let controlled = |done: Done| {
            let filter = ObjectFilter::ControlledBy(PlayerSet::One(player.clone()));
            self.this_way_set(done, Before, Some(filter)).map(some)
        };
        let took = || -> Result<Condition> {
            let result = self
                .took
                .clone()
                .ok_or_else(|| invalid("no player chose to act in this scope"))?;
            Ok(Condition::PlayerTook {
                player: player.clone(),
                result,
            })
        };
        Ok(Some(match value {
            S::PlayerWhoTookActionThisWay => took()?,
            S::DidntTakeAction => Condition::Not(Box::new(took()?)),
            S::SacrificedAPermanentThisWay(filter) => some(self.moved_by(
                Done::Sacrificed,
                Before,
                player,
                Some(self.filter(filter)?),
            )?),
            S::DidntSacrificeAPermanentThisWay(filter) => none(self.moved_by(
                Done::Sacrificed,
                Before,
                player,
                Some(self.filter(filter)?),
            )?),
            S::DiscardedACardThisWay => some(self.moved_by(Done::Discarded, After, player, None)?),
            S::DiscardedACardOfTypeThisWay(filter) => some(self.moved_by(
                Done::Discarded,
                After,
                player,
                Some(self.card_filter(filter)?),
            )?),
            S::DidntDiscardedACardOfTypeThisWay(filter) => none(self.moved_by(
                Done::Discarded,
                After,
                player,
                Some(self.card_filter(filter)?),
            )?),
            S::ExiledACardThisWay => some(self.moved_by(Done::Exiled, After, player, None)?),
            S::ControlledAPermanentDestroyedThisWay => controlled(Done::Destroyed)?,
            S::ControlledAPermanentExiledThisWay => controlled(Done::Exiled)?,
            _ => return Ok(None),
        }))
    }

    /// Those who didn't are drawn from those who were offered it.
    fn offered(&self) -> Result<PlayerSet> {
        self.offered
            .clone()
            .ok_or_else(|| invalid("no batch of players acted in this scope"))
    }
}

/// The instruction that says a body ran to its end.
fn last_effect(body: &Program) -> Option<ResultId> {
    body.instructions.iter().rev().find_map(|instruction| match instruction {
        Instruction::Effect { result, .. } => Some(result.clone()),
        Instruction::Group(body) | Instruction::Optional { body, .. } => last_effect(body),
        _ => None,
    })
}
