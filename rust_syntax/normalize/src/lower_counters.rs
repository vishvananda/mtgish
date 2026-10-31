impl Context {
    fn removal(
        &self,
        recipients: Recipients,
        kind: Option<&source::CounterType>,
        amount: CounterAmount,
        choose: Option<Count>,
        actor: &PlayerSet,
    ) -> Result<CounterRemoval> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("counter removal requires one explicit actor"));
        };
        let selection = choose.map(|count| SelectionChoice {
            player: player.clone(),
            count,
        });
        Ok(CounterRemoval {
            player: player.clone(),
            recipients,
            kind: kind.map(crate::counter::lower),
            amount,
            selection,
        })
    }

    fn remove_counter_cost(&self, value: &source::RemoveCountersCost) -> Result<CounterRemoval> {
        use source::RemoveCountersCost as S;
        let one = || CounterAmount::Count(Count::Exactly(Expr::Constant(1)));
        let (objects, kind, amount, choose) = match value {
            S::ACounterFromPermanent(p) => (ObjectSet::One(self.object(p)?), None, one(), None),
            S::ACounterOfTypeFromPermanent(k, p) => {
                (ObjectSet::One(self.object(p)?), Some(k), one(), None)
            }
            S::ACounterFromAPermanent(p) => (
                self.objects(p)?,
                None,
                one(),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::ACounterOfTypeFromAPermanent(k, p) => (
                self.objects(p)?,
                Some(k),
                one(),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::NumberCountersFromPermanent(n, p) => (
                ObjectSet::One(self.object(p)?),
                None,
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::NumberCountersFromAPermanent(n, p) => (
                self.objects(p)?,
                None,
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::NumberCountersOfTypeFromPermanent(n, k, p) => (
                ObjectSet::One(self.object(p)?),
                Some(k),
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::AnyNumberOfCountersOfTypeFromPermanent(k, p) => (
                ObjectSet::One(self.object(p)?),
                Some(k),
                CounterAmount::Count(Count::AnyNumber),
                None,
            ),
            S::NumberOrMoreCountersOfTypeFromPermanent(n, k, p) => (
                ObjectSet::One(self.object(p)?),
                Some(k),
                CounterAmount::Count(Count::AtLeast(self.number(n)?)),
                None,
            ),
            S::AllCountersOfTypeFromPermanent(k, p) => (
                ObjectSet::One(self.object(p)?),
                Some(k),
                CounterAmount::All,
                None,
            ),
            S::ACounterOfTypeFromEachOfNumberPermanents(k, n, p) => (
                self.objects(p)?,
                Some(k),
                one(),
                Some(Count::Exactly(self.number(n)?)),
            ),
            S::NumberCountersFromAmongPermanents(n, p) => (
                self.objects(p)?,
                None,
                CounterAmount::AmongThem(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::NumberCountersOfTypeFromAmongPermanents(n, k, p) => (
                self.objects(p)?,
                Some(k),
                CounterAmount::AmongThem(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::NumberOrMoreCountersOfTypeFromAmongPermanents(n, k, p) => (
                self.objects(p)?,
                Some(k),
                CounterAmount::AmongThem(Count::AtLeast(self.number(n)?)),
                None,
            ),
            S::AnyNumberOfCountersOfTypeFromAmongPermanents(k, p) => (
                self.objects(p)?,
                Some(k),
                CounterAmount::AmongThem(Count::AnyNumber),
                None,
            ),
            _ => return Err(unsupported(value)),
        };
        self.removal(
            Recipients::Objects(objects),
            kind,
            amount,
            choose,
            &PlayerSet::One(
                self.payer
                    .clone()
                    .unwrap_or(self.player(&source::Player::You)?),
            ),
        )
    }

    fn remove_counter_action(
        &self,
        value: &source::RemoveCountersAction,
        actor: &PlayerSet,
    ) -> Result<CounterRemoval> {
        use source::RemoveCountersAction as S;
        let one = || CounterAmount::Count(Count::Exactly(Expr::Constant(1)));
        let object = |p| -> Result<_> { Ok(Recipients::Objects(ObjectSet::One(self.object(p)?))) };
        let player = |p| -> Result<_> { Ok(Recipients::Players(PlayerSet::One(self.player(p)?))) };
        let each = |p| -> Result<_> { Ok(Recipients::Objects(self.objects(p)?)) };
        let exiled =
            |c| -> Result<_> { Ok(Recipients::Objects(ObjectSet::One(self.exiled_card(c)?))) };
        let exiled_each = |c| -> Result<_> { Ok(Recipients::Objects(self.exile_cards(c)?)) };
        let exactly = |n| -> Result<_> {
            Ok(CounterAmount::Count(Count::Exactly(self.number(n)?)))
        };
        let (recipients, kind, amount, choose) = match value {
            S::ACounterFromCardInExile(c) => (exiled(c)?, None, one(), None),
            S::ACounterOfTypeFromCardInExile(k, c) => (exiled(c)?, Some(k), one(), None),
            S::NumberCountersOfTypeFromCardInExile(n, k, c) => {
                (exiled(c)?, Some(k), exactly(n)?, None)
            }
            S::ACounterOfTypeFromEachCardInExile(k, c) => {
                (exiled_each(c)?, Some(k), one(), None)
            }
            S::NumberCountersOfTypeFromEachCardInExile(n, k, c) => {
                (exiled_each(c)?, Some(k), exactly(n)?, None)
            }
            S::NumberCountersOfTypeFromACardInExile(n, k, c) => (
                exiled_each(c)?,
                Some(k),
                exactly(n)?,
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::ACounterFromPermanent(p) => (object(p)?, None, one(), None),
            S::ACounterFromPlayer(p) => (player(p)?, None, one(), None),
            S::ACounterOfTypeFromPermanent(k, p) => (object(p)?, Some(k), one(), None),
            S::ACounterOfTypeFromEachPermanent(k, p) => (each(p)?, Some(k), one(), None),
            S::ACounterOfTypeFromEachOfAnyNumberOfPermanents(k, p) => {
                (each(p)?, Some(k), one(), Some(Count::AnyNumber))
            }
            S::AllCountersFromPermanent(p) => (object(p)?, None, CounterAmount::All, None),
            S::AllCountersFromPlayer(p) => (player(p)?, None, CounterAmount::All, None),
            S::AllCountersFromEachPermanent(p) => (each(p)?, None, CounterAmount::All, None),
            S::AllCountersFromEachPlayer(p) => (
                Recipients::Players(self.players(p)?),
                None,
                CounterAmount::All,
                None,
            ),
            S::AllCountersOfTypeFromPermanent(k, p) => {
                (object(p)?, Some(k), CounterAmount::All, None)
            }
            S::AllCountersOfTypeFromPlayer(k, p) => (player(p)?, Some(k), CounterAmount::All, None),
            S::AllCountersOfTypeFromEachPermanent(k, p) => {
                (each(p)?, Some(k), CounterAmount::All, None)
            }
            S::AllCountersOfTypeFromAPermanent(k, p) => (
                each(p)?,
                Some(k),
                CounterAmount::All,
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::AnyNumberOfCountersFromPermanent(p) => (
                object(p)?,
                None,
                CounterAmount::Count(Count::AnyNumber),
                None,
            ),
            S::NumberCountersFromPermanent(n, p) => (
                object(p)?,
                None,
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::NumberCountersOfTypeFromPermanent(n, k, p) => (
                object(p)?,
                Some(k),
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::NumberCountersOfTypeFromEachPermanent(n, k, p) => (
                each(p)?,
                Some(k),
                CounterAmount::Count(Count::Exactly(self.number(n)?)),
                None,
            ),
            S::UptoNumberCountersFromPermanent(n, p) => (
                object(p)?,
                None,
                CounterAmount::Count(Count::UpTo(self.number(n)?)),
                None,
            ),
            S::UptoNumberCountersFromPlayer(n, p) => (
                player(p)?,
                None,
                CounterAmount::Count(Count::UpTo(self.number(n)?)),
                None,
            ),
            S::UptoNumberCountersOfTypeFromPermanent(n, k, p) => (
                object(p)?,
                Some(k),
                CounterAmount::Count(Count::UpTo(self.number(n)?)),
                None,
            ),
            _ => return Err(unsupported(value)),
        };
        self.removal(recipients, kind, amount, choose, actor)
    }
}
