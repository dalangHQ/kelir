// see: cans/03-sdk-architecture.md#Rust-SDK

use std::marker::PhantomData;

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
// TODO (L2): cans/03 spells the bound as Plugins: Has<T>; the stable-coherent form carries the list position as an inferred Index parameter (E0119/E0207 block the single-parameter blanket).
pub trait Has<T, Index> {
    fn capability(&self) -> &T;
}

impl<T, Tail> Has<T, Here> for HCons<T, Tail> {
    fn capability(&self) -> &T {
        &self.head
    }
}

impl<Head, T, Index, Tail> Has<T, There<Index>> for HCons<Head, Tail>
where
    Tail: Has<T, Index>,
{
    fn capability(&self) -> &T {
        self.tail.capability()
    }
}

// see: cans/03-sdk-architecture.md#Params-typed-views
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoParams;
