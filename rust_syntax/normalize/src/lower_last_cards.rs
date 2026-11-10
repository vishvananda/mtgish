impl Context {
    /// Look at the top of a planar deck and put those cards back, as Forge's `Dig`
    /// with `SourceZone$ PlanarDeck`. Choices and moves read the look's own result.
    fn inspect_planar_deck(
        &mut self,
        amount: &source::GameNumber,
        actions: &[source::ReplacementActionWouldPlaneswalkLookAtTheTopNumberCardsOfPlanarDeckAction],
        player: &PlayerRef,
    ) -> Result<Instruction> {
        use source::ReplacementActionWouldPlaneswalkLookAtTheTopNumberCardsOfPlanarDeckAction as S;
        let deck = self.private_cards(
            EventZone::PlanarDeck,
            PlayerSet::One(player.clone()),
            ObjectFilter::Any,
        );
        let look = self.effect(Operation::LookAt {
            player: player.clone(),
            objects: ObjectSet::Take {
                objects: Box::new(deck),
                count: self.number(amount)?,
                from: SequenceEnd::Top,
            },
        });
        let Instruction::Effect { result, .. } = &look else {
            unreachable!()
        };
        let observed = ObjectSet::Observed(result.clone());
        let mut placed: Vec<ObjectSet> = vec![];
        let mut instructions = vec![look];
        for action in actions {
            let remaining = ObjectSet::InZone {
                objects: Box::new(if placed.is_empty() {
                    observed.clone()
                } else {
                    ObjectSet::Difference {
                        objects: Box::new(observed.clone()),
                        excluding: Box::new(ObjectSet::Union(placed.clone())),
                    }
                }),
                zone: EventZone::PlanarDeck,
            };
            let (objects, end) = match action {
                S::PutACardOnBottom => (
                    self.inspection_choice(
                        player,
                        remaining,
                        ObjectFilter::Any,
                        Count::Exactly(Expr::Constant(1)),
                        vec![],
                        &mut instructions,
                    ),
                    SequenceEnd::Bottom,
                ),
                S::PutTheRemainingCardOnTopInAnyOrder => (remaining, SequenceEnd::Top),
            };
            let put = self.effect(crate::movement::move_plain(
                objects,
                Destination::OwnersPlanarDeck(end),
            ));
            let Instruction::Effect { result, .. } = &put else {
                unreachable!()
            };
            placed.push(ObjectSet::Transition {
                result: result.clone(),
                phase: ObjectPhase::Before,
            });
            instructions.push(put);
        }
        Ok(Instruction::Group(Program { instructions }))
    }
}
