//! Source-only entry points into the shared coin protocol inference.
use crate::ir::*;
pub use oracle_ir_contract::coin_policies::*;
use std::collections::BTreeSet;
#[cfg(feature = "legacy")]
pub(crate) fn source_card(card: &mut Card) -> Result<(), String> {
    if !has_coins(card) {
        return Ok(());
    }
    let mut wrapped = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: Layout::Card,
        faces: vec![card.clone()],
        data: vec![],
    };
    infer(&mut wrapped, true, BTreeSet::new())?;
    *card = wrapped.faces.remove(0);
    Ok(())
}
#[cfg(feature = "legacy")]
pub(crate) fn source_unified(card: &mut UnifiedCard) -> Result<(), String> {
    infer(card, true, BTreeSet::new())
}

#[cfg(feature = "legacy")]
pub(crate) fn source_operand(operand: &mut Operand) -> Result<(), String> {
    if !has_coins(operand) {
        return Ok(());
    }
    // Metadata-only operands still have private program/ability scopes. Use
    // the same typed document resolver instead of a separate ID heuristic.
    let mut wrapped = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: Layout::Extension {
            namespace: "org.mtgish.source".into(),
            name: "Operand".into(),
        },
        faces: vec![],
        data: vec![Field {
            name: "Value".into(),
            value: operand.clone(),
        }],
    };
    infer(&mut wrapped, true, BTreeSet::new())?;
    *operand = wrapped.data.remove(0).value;
    Ok(())
}
fn has_coins<T: crate::visit::Walk>(value: &T) -> bool {
    use crate::visit::Visitor;
    struct Find(bool);
    impl Visitor for Find {
        fn operation(&mut self, op: &Operation) -> bool {
            if matches!(
                op,
                Operation::FlipCoin { .. }
                    | Operation::FlipCoins { .. }
                    | Operation::FlipCoinSeries { .. }
            ) {
                self.0 = true;
            }
            !self.0
        }
    }
    let mut find = Find(false);
    value.walk(&mut find);
    find.0
}
