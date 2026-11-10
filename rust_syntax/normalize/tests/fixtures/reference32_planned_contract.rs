// Compile-only adapters for interfaces the engine has not implemented. Every
// method panics if executed: this proves Rust types, never gameplay behavior.
mod contract {
    use cantrip_api::*;
    pub trait DamageLoyaltyFloor {
        fn leave_loyalty(&mut self, minimum: u32);
    }
    impl DamageLoyaltyFloor for DamageEvent {
        fn leave_loyalty(&mut self, _: u32) { unreachable!("unimplemented engine contract") }
    }
    pub trait CreationExileQuery {
        fn creation_linked_exiled(&self, source: ObjectId) -> Vec<ObjectId>;
    }
    impl<T: Query + ?Sized> CreationExileQuery for T {
        fn creation_linked_exiled(&self, _: ObjectId) -> Vec<ObjectId> { unreachable!("unimplemented engine contract") }
    }
    pub trait CreationExileGame {
        fn create_token_copies_linked_to_exiled(&mut self, values: Copiable, controller: PlayerId, count: u32, tapped: bool, cards: &[ObjectId]) -> Result<Vec<ObjectId>, Suspend>;
    }
    impl<T: Game + ?Sized> CreationExileGame for T {
        fn create_token_copies_linked_to_exiled(&mut self, _: Copiable, _: PlayerId, _: u32, _: bool, _: &[ObjectId]) -> Result<Vec<ObjectId>, Suspend> { unreachable!("unimplemented engine contract") }
    }
}
