// see: cans/03-sdk-architecture.md#Rust-SDK

use std::marker::PhantomData;

use kelir_interface::handlers_plugin::Has;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HNil;

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

// see: cans/03-sdk-architecture.md#Params-typed-views
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoParams;
