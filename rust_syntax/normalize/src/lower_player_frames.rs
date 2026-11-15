// Source player-action wrappers bind the doer without rebinding Oracle's You.
impl Context {
    fn player_action_frame(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Result<Instruction>> {
        use source::Action as A;
        let (players, any_one, optional, actions) = match value {
            A::MayHavePlayerAction(player, action) => (
                self.player(player).map(PlayerSet::One),
                false,
                true,
                vec![action.as_ref().clone()],
            ),
            A::APlayerMayAction(players, action) => (
                self.players(players),
                true,
                true,
                vec![action.as_ref().clone()],
            ),
            A::APlayerActions(players, actions) => {
                (self.players(players), true, false, actions.clone())
            }
            A::ActionForEachPlayerInTurnOrder(players, actions) => {
                (self.players(players), false, false, actions.clone())
            }
            _ => return None,
        };
        Some((|| {
            let players = players?;
            let mut child = self.nested_program();
            // A "repeat this process for each player" loop binds EachablePlayer
            // but leaves implicit actions with the original actor (Protection
            // Racket reveals YOUR library). Player-action policies change doer.
            let body_actor = if matches!(value, A::ActionForEachPlayerInTurnOrder(..)) {
                actor.clone()
            } else {
                child.acting = Some(PlayerRef::Acting);
                PlayerSet::One(PlayerRef::Acting)
            };
            child.iterated = Some(PlayerRef::Acting);
            let body = if let A::APlayerActions(_, actions) = value {
                // Keep the Source policy's separate action operands while
                // sharing the same acting-player scope and result allocator.
                actions
                    .iter()
                    .map(|action| {
                        child
                            .action_list(std::slice::from_ref(action), &body_actor)
                            .map(Operand::Program)
                    })
                    .collect::<Result<Vec<_>>>()?
            } else {
                vec![Operand::Program(child.action_list(&actions, &body_actor)?)]
            };
            self.result = child.result;
            Ok(self.effect(Operation::PlayersDo {
                players,
                any_one,
                optional,
                once_each_turn: false,
                body,
                otherwise: vec![],
            }))
        })())
    }
}
