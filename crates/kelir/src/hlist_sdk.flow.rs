// see: cans/03-sdk-architecture.md#Rust-SDK

use std::marker::PhantomData;

use kelir_interface::handlers_plugin::{Capability, Has};

pub use kelir_interface::plugin_model::HNil;

#[derive(Clone)]
pub struct HCons<Head, Tail> {
    pub head: Head,
    pub tail: Tail,
}

// see: cans/03-sdk-architecture.md#Rust-SDK
pub trait HList: Clone + Send + Sync + 'static {
    type Tail: HList;

    fn tail_value(self) -> Self::Tail;
}

impl HList for HNil {
    type Tail = HNil;

    fn tail_value(self) -> Self::Tail {
        self
    }
}

impl<Head, Tail> HList for HCons<Head, Tail>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList,
{
    type Tail = Tail;

    fn tail_value(self) -> Self::Tail {
        self.tail
    }
}

// see: cans/03-sdk-architecture.md#Rust-SDK
pub trait Satisfies<Other> {}

// TODO (L3) [plugin_engine]: order-insensitive Satisfies is E0207-blocked as written (Has<Need, Index> leaves Index unconstrained); the witness-threaded arity-2 form compiles but callers cannot name the witness, so it needs a builder-API contract change, not a lane fix.; slop: plugin order stays positional, mounting out of dependency order fails the build
impl<Plugins: HList> Satisfies<HNil> for Plugins {}

impl<Head, Tail, OtherTail> Satisfies<HCons<Head, OtherTail>> for HCons<Head, Tail>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList + Satisfies<OtherTail>,
{
}

// see: cans/02-plugin-model.md#Compile-time-prerequisites
impl<Head, Tail, OtherTail> Satisfies<kelir_interface::plugin_model::HCons<Head, OtherTail>>
    for HCons<Head, Tail>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList + Satisfies<OtherTail>,
{
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub struct Here;

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
pub struct There<Index>(PhantomData<fn() -> Index>);

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
trait Lookup<T, Index> {
    fn lookup(&self) -> &T;
}

impl<T, Tail> Lookup<T, Here> for HCons<T, Tail> {
    fn lookup(&self) -> &T {
        &self.head
    }
}

impl<Head, T, Index, Tail> Lookup<T, There<Index>> for HCons<Head, Tail>
where
    Tail: Lookup<T, Index>,
{
    fn lookup(&self) -> &T {
        self.tail.lookup()
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl<Head, Tail, T, Index> Has<T, Index> for HCons<Head, Tail>
where
    HCons<Head, Tail>: Lookup<T, Index>,
{
    fn get(&self) -> &T {
        self.lookup()
    }
}

// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
impl<Head, Tail, P> Capability<P> for HCons<Head, Tail>
where
    Head: Clone + Send + Sync + 'static,
    Tail: HList + Capability<P>,
    P: 'static,
{
    fn capability(&self) -> Option<&P> {
        self.tail.capability().or_else(|| {
            let head: &dyn std::any::Any = &self.head;
            head.downcast_ref::<P>()
        })
    }
}

// see: cans/03-sdk-architecture.md#Params-typed-views
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoParams;
