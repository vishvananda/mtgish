//! Which views of one result the current resolution and its closures consume.
use crate::{
    ir::*,
    visit::{Visitor, Walk},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResultReads {
    pub objects: bool,
    pub membership: bool,
    pub graveyard_count: bool,
    pub kept_on_top: bool,
}

impl ResultReads {
    pub fn instructions(after: &[Instruction], result: &ResultId) -> Self {
        let mut reader = Reader {
            result: result.0,
            reads: Self::default(),
        };
        for instruction in after {
            instruction.walk(&mut reader);
        }
        reader.reads
    }
    pub fn graveyard(self) -> bool {
        self.membership || self.graveyard_count
    }
}

struct Reader {
    result: u32,
    reads: ResultReads,
}
impl Visitor for Reader {
    // Granted/copy/token abilities are declarations with independent result
    // namespaces. Created delayed/reflexive/replacement Programs are lexical
    // closures, so their bodies are deliberately traversed instead.
    fn ability(&mut self, _: &Ability) -> bool {
        false
    }
    fn object_filter(&mut self, filter: &ObjectFilter) -> bool {
        if filter.named_result().is_some_and(|r| r.0 == self.result) {
            self.reads.membership = true;
            self.reads.objects = true;
        }
        true
    }
    fn object_set(&mut self, objects: &ObjectSet) -> bool {
        if matches!(objects, ObjectSet::Recorded(r) | ObjectSet::Affected(r) | ObjectSet::Transition { result:r, .. } if r.0 == self.result)
        {
            self.reads.objects = true;
        }
        true
    }
    fn object_ref(&mut self, object: &ObjectRef) -> bool {
        if matches!(object, ObjectRef::Recorded {result:r,..} if r.0 == self.result) {
            self.reads.objects = true;
        }
        true
    }
    fn operation(&mut self, operation: &Operation) -> bool {
        if matches!(operation, Operation::JoinResults {results} if results.iter().any(|r| r.0 == self.result))
        {
            self.reads.objects = true;
        }
        true
    }
    fn expr(&mut self, expr: &Expr) -> bool {
        if matches!(expr, Expr::SurveilKeptOnTop(r) if r.0 == self.result) {
            self.reads.kept_on_top = true;
        }
        if matches!(expr, Expr::Earlier {what:EarlierObjects::PutInGraveyard,measure:ObjectMeasure::Count,result:Some(r),..} if r.0 == self.result)
        {
            self.reads.graveyard_count = true;
        }
        true
    }
}
