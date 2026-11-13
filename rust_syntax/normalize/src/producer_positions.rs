//! Equivalent ordered-zone queries in fresh output. The position is applied
//! after an inner filter; an outer Matching remains after the position.
use crate::{
    ir::*,
    visit::{VisitorMut, WalkMut},
};

fn ordered_one(objects: &ObjectSet) -> bool {
    matches!(
        objects,
        ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::Library | EventZone::Graveyard,
                players: PlayerSet::One(_)
            },
            kind: ObjectKind::Card,
            ..
        }
    )
}
pub(super) fn scalar(objects: &ObjectSet) -> ObjectRef {
    match objects {
        ObjectSet::One(object) => object.clone(),
        ObjectSet::Transition { result, phase } => ObjectRef::Result {
            result: result.clone(),
            phase: phase.clone(),
            index: 0,
        },
        ObjectSet::Take {
            objects,
            count: Expr::Constant(1),
            from: SequenceEnd::Top,
        } if ordered_one(objects) => ObjectRef::Query {
            objects: objects.clone(),
            index: 0,
        },
        _ => ObjectRef::Query {
            objects: Box::new(objects.clone()),
            index: 0,
        },
    }
}
struct Positions;
impl VisitorMut for Positions {
    fn object_ref_end(&mut self, object: &mut ObjectRef) {
        if let ObjectRef::Query { objects, index: 0 } = object {
            if let ObjectSet::Take {
                objects: pool,
                count: Expr::Constant(1),
                from: SequenceEnd::Top,
            } = objects.as_ref()
            {
                if ordered_one(pool) {
                    *object = ObjectRef::Query {
                        objects: pool.clone(),
                        index: 0,
                    };
                }
            }
        }
    }
    fn object_set_end(&mut self, objects: &mut ObjectSet) {
        match objects {
            ObjectSet::LibraryTops(players @ PlayerSet::One(_)) => {
                *objects = ObjectSet::Take {
                    objects: Box::new(ObjectSet::Zone {
                        scope: ZoneScope::Players {
                            zone: EventZone::Library,
                            players: players.clone(),
                        },
                        kind: ObjectKind::Card,
                        filter: ObjectFilter::Any,
                    }),
                    count: Expr::Constant(1),
                    from: SequenceEnd::Top,
                };
            }
            ObjectSet::One(ObjectRef::Query {
                objects: pool,
                index: 0,
            }) if ordered_one(pool) => {
                *objects = ObjectSet::Take {
                    objects: pool.clone(),
                    count: Expr::Constant(1),
                    from: SequenceEnd::Top,
                };
            }
            ObjectSet::One(ObjectRef::Query {
                objects: pool,
                index: 0,
            }) => {
                if let ObjectSet::Take {
                    objects: zone,
                    count: Expr::Constant(1),
                    ..
                } = pool.as_ref()
                {
                    if ordered_one(zone) {
                        *objects = pool.as_ref().clone();
                    }
                }
            }
            _ => {}
        }
    }
}
pub(super) fn normalize<T: WalkMut>(value: &mut T) {
    value.walk_mut(&mut Positions);
}
